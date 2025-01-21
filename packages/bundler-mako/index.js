const path = require('path');
const fs = require('fs');
const http = require('http');
const assert = require('assert');
const { createProxy, createHttpsServer } = require('@umijs/bundler-utils');
const lodash = require('lodash');
const chalk = require('chalk');
const { parseTsconfig } = require('get-tsconfig');
const {
  createProxyMiddleware,
} = require('@umijs/bundler-utils/compiled/http-proxy-middleware');

exports.build = async function (opts) {
  assert(opts, 'opts should be supplied');
  const {
    cwd,
    onBuildComplete,
    // 尚有不支持的配置项，checkConfig 会根据情况做报错、警告及忽略
    // 详见：https://github.com/umijs/mako/issues/611
  } = opts;
  checkConfig(opts);

  const makoConfig = await getMakoConfig(opts);
  const originStats = makoConfig.stats;
  // always enable stats to provide json for onBuildComplete hook
  makoConfig.stats = {
    modules: false,
  };
  makoConfig.mode = 'production';
  makoConfig.hash = !!opts.config.hash;
  if (makoConfig.hash) {
    makoConfig.moduleIdStrategy = 'hashed';
  }

  makoConfig.plugins = makoConfig.plugins || [];

  let statsJson;
  makoConfig.plugins.push({
    name: 'mako-stats',
    generateEnd: (args) => {
      statsJson = args.stats;
    },
  });

  const { build } = require('@umijs/mako');
  try {
    await build({
      root: cwd,
      config: makoConfig,
      watch: opts.watch || false,
    });
  } catch (e) {
    console.error(e.message);
    opts.onBuildError?.(e);
    const err = new Error('Build with mako failed.');
    err.stack = null;
    throw err;
  }

  const stats = {
    compilation: {
      ...statsJson,
      // convert assets to [key: value] for compilation data
      assets: statsJson.assets.reduce(
        (acc, asset) => ({
          ...acc,
          [asset.name]: { size: asset.size },
        }),
        {},
      ),
    },
    hasErrors: () => false,
  };
  await onBuildComplete({
    err: null,
    stats,
  });
  return stats;
};

exports.dev = async function (opts) {
  assert(opts, 'opts should be supplied');
  checkConfig(opts);
  const express = require('express');
  const proxy = require('express-http-proxy');
  const app = express();
  const port = opts.port || 8000;
  const hmrPort = opts.port + 1;

  // cors
  app.use(
    require('cors')({
      origin: true,
      methods: ['GET', 'HEAD', 'PUT', 'POST', 'PATCH', 'DELETE', 'OPTIONS'],
      credentials: true,
    }),
  );
  // compression
  app.use(require('compression')());
  // Provides the ability to execute custom middleware prior to all other middleware internally within the server.
  if (opts.onBeforeMiddleware) {
    opts.onBeforeMiddleware(app);
  }
  // before middlewares
  (opts.beforeMiddlewares || []).forEach((m) => app.use(m));

  // proxy ws to mako server
  const wsProxy = createProxyMiddleware({
    // mako server in the same host so hard code is ok
    target: `http://127.0.0.1:${hmrPort}`,
    ws: true,
    logLevel: 'silent',
  });
  app.use('/__/hmr-ws', wsProxy);

  // proxy
  if (opts.config.proxy) {
    createProxy(opts.config.proxy, app);
  }

  app.use(
    proxy(`http://127.0.0.1:${hmrPort}`, {
      proxyReqOptDecorator: function (proxyReqOpts) {
        // keep alive is on by default https://nodejs.org/docs/latest/api/http.html#httpglobalagent
        // 禁用 keep-alive
        proxyReqOpts.agent = false;
        return proxyReqOpts;
      },
      filter: function (req, res) {
        return req.method == 'GET' || req.method == 'HEAD';
      },
      skipToNextHandlerFilter: function (proxyRes) {
        return proxyRes.statusCode !== 200;
      },
    }),
  );

  // after middlewares
  (opts.afterMiddlewares || []).forEach((m) => {
    // TODO: FIXME
    app.use(m.toString().includes('{ compiler }') ? m({}) : m);
  });
  // history fallback
  app.use(
    require('connect-history-api-fallback')({
      index: '/',
    }),
  );
  // create server
  let server;
  const httpsOpts = opts.config.https;
  if (httpsOpts) {
    httpsOpts.hosts ||= lodash.uniq(
      [
        ...(httpsOpts.hosts || []),
        // always add localhost, 127.0.0.1, ip and host
        '127.0.0.1',
        'localhost',
        opts.ip,
        opts.host !== '0.0.0.0' && opts.host,
      ].filter(Boolean),
    );
    server = await createHttpsServer(app, httpsOpts);
  } else {
    server = http.createServer(app);
  }
  server.listen(port, () => {
    const protocol = opts.config.https ? 'https:' : 'http:';
    const banner = getDevBanner(protocol, opts.host, port, opts.ip);
    console.log(banner);
  });
  // prevent first websocket auto disconnected
  // ref https://github.com/chimurai/http-proxy-middleware#external-websocket-upgrade
  server.on('upgrade', wsProxy.upgrade);

  // mako dev
  const { build } = require('@umijs/mako');
  const makoConfig = await getMakoConfig(opts);
  if (process.env.HMR === 'none') {
    makoConfig.hmr = false;
  } else {
    makoConfig.hmr = {};
  }
  makoConfig.devServer = { port: hmrPort, host: opts.host };
  makoConfig.plugins = makoConfig.plugins || [];
  makoConfig.plugins.push({
    name: 'mako-dev',
    generateEnd: (args) => {
      opts.onDevCompileDone(args);
    },
  });
  const cwd = opts.cwd;
  try {
    await build({
      root: cwd,
      config: makoConfig,
      watch: true,
    });
  } catch (e) {
    opts.onBuildError?.(e);
    console.error(e.message);
    const err = new Error('Build with mako failed.');
    err.stack = null;
    throw err;
  }
};

function getDevBanner(protocol, host, port, ip) {
  const hostStr = host === '0.0.0.0' ? 'localhost' : host;
  const messages = [];
  messages.push('  App listening at:');
  messages.push(
    `  - Local:   ${chalk.cyan(`${protocol}//${hostStr}:${port}`)}`,
  );
  messages.push(`  - Network: ${chalk.cyan(`${protocol}//${ip}:${port}`)}`);
  return messages.join('\n');
}

function checkConfig(opts) {
  // 构建不支持的配置项，会直接报错
  const unsupportedKeys = [
    // 不支持 MFSU
    'mfsu',
    // 暂不支持多 entry
    'mpa',
  ];

  // 处理构建不支持的配置项
  unsupportedKeys.forEach((key) => {
    assert(!opts.config[key], `${key} is not supported in Mako bundler`);
  });

  // 支持透传给 mako 的配置
  const supportMakoConfigKeys = [
    'plugins',
    'px2rem',
    'experimental',
    'flexBugs',
    'optimization',
    'sass',
    'autoCSSModules',
    'moduleIdStrategy',
  ];
  // umi mako config
  const { mako } = opts.config;
  Object.keys(mako).forEach((key) => {
    assert(
      supportMakoConfigKeys.includes(key),
      `umi config mako.${key} is not supported`,
    );
  });
  // 暂不支持 { from, to } 格式
  const { copy } = opts.config;
  if (copy) {
    for (const item of copy) {
      assert(
        typeof item === 'string',
        `copy config item must be string in Mako bundler, but got ${item}`,
      );
    }
  }

  // 不支持数组 externals
  if (Array.isArray(opts.config.externals)) {
    throw new Error('externals array is not supported in Mako bundler');
  }

  // 对 externals 的具体值做判定
  Object.values(opts.config.externals || {}).forEach((v) => {
    if (Array.isArray(v) && (v.length !== 2 || !v[0].startsWith('script '))) {
      // 不支持非 script 的 [string] externals
      throw new Error(
        `externals [string] value only can be ['script {url}', '{root}'] in Mako bundler`,
      );
    } else if (
      typeof v === 'object' &&
      !lodash.isPlainObject(v) &&
      !Array.isArray(v)
    ) {
      throw new Error(
        'externals non-plain object value is not supported in Mako bundler',
      );
    } else if (typeof v === 'function') {
      throw new Error(
        'externals function value is not supported in Mako bundler',
      );
    } else if (v instanceof RegExp) {
      throw new Error(
        'externals RegExp value is not supported in Mako bundler',
      );
    } else if (
      typeof v === 'string' &&
      // allow prefix window type
      // ex. `window antd`
      !/^window\s+/.test(v) &&
      // allow normal string value without type prefix
      // ex. `antd` or `antd.Button` or `antd['Button']` or `window.antd`
      !/^\S+$/.test(v)
    ) {
      // throw error for other type prefixes
      // ex. `commonjs`、`var 1 + 1`、`global`
      throw new Error(
        `externals string value prefix \`${
          v.split(' ')[0]
        } \` is not supported in Mako bundler`,
      );
    }
  });

  // 不支持但对构建影响不明确的配置项，会统一警告
  const riskyKeys = [
    'config.autoprefixer',
    'config.cssPublicPath',
    'config.cssLoader',
    'config.cssLoaderModules',
    'config.classPropertiesLoose',
    'config.extraPostCSSPlugins',
    'config.postcssLoader',
    'config.styleLoader',
    'config.stylusLoader',
    'config.chainWebpack',
  ];
  // 收集警告的配置项
  const warningKeys = [];

  riskyKeys.forEach((key) => {
    if (lodash.get(opts, key)) {
      warningKeys.push(key.split('.').pop());
    }
  });

  // 不支持 mdx 子配置
  if (opts.config.mdx && Object.keys(opts.config.mdx).length) {
    warningKeys.push('mdx');
  }

  // 不支持 lessLoader 的部分配置
  if (opts.config.lessLoader) {
    lodash
      .difference(Object.keys(opts.config.lessLoader), [
        'javascriptEnabled',
        'modifyVars',
        'globalVars',
        'math',
        'plugins',
      ])
      .forEach((k) => {
        warningKeys.push(`lessLoader.${k}`);
      });
  }

  // 不支持内置 babel preset 以外的其他预设
  ['beforeBabelPresets', 'extraBabelPresets', 'config.extraBabelPresets']
    .reduce((acc, key) => acc.concat(lodash.get(opts, key) || []), [])
    .some((p) => {
      if (!p.plugins?.[0]?.[1]?.onCheckCode) {
        warningKeys.push('extraBabelPresets');
        return true;
      }
    });

  // 不支持除 babel-plugin-import 以外的插件
  ['beforeBabelPlugins', 'extraBabelPlugins', 'config.extraBabelPlugins']
    .reduce((acc, key) => acc.concat(lodash.get(opts, key) || []), [])
    .some((p) => {
      const isImportPlugin = /^import$|babel-plugin-import/.test(p[0]);
      const isEmotionPlugin = p === '@emotion' || p === '@emotion/babel-plugin';
      const isDynamicImportNode = /^(babel-plugin-)?dynamic-import-node$/.test(
        p,
      );
      if (!isImportPlugin && !isEmotionPlugin && !isDynamicImportNode) {
        warningKeys.push('extraBabelPlugins');
        return true;
      }
    });

  // 不支持非字符串形式的 theme
  Object.values(opts.config.theme || {})
    .reduce((ret, v) => {
      const type = typeof v;
      if (type !== 'string') ret.add(type);
      return ret;
    }, new Set())
    .forEach((type) => {
      warningKeys.push(`theme.[${type} value]`);
    });

  if (warningKeys.length) {
    console.warn(
      chalk.yellow(
        `
=====================================================================================================

   █████   ███   █████   █████████   ███████████   ██████   █████ █████ ██████   █████   █████████
  ░░███   ░███  ░░███   ███░░░░░███ ░░███░░░░░███ ░░██████ ░░███ ░░███ ░░██████ ░░███   ███░░░░░███
   ░███   ░███   ░███  ░███    ░███  ░███    ░███  ░███░███ ░███  ░███  ░███░███ ░███  ███     ░░░
   ░███   ░███   ░███  ░███████████  ░██████████   ░███░░███░███  ░███  ░███░░███░███ ░███
   ░░███  █████  ███   ░███░░░░░███  ░███░░░░░███  ░███ ░░██████  ░███  ░███ ░░██████ ░███    █████
    ░░░█████░█████░    ░███    ░███  ░███    ░███  ░███  ░░█████  ░███  ░███  ░░█████ ░░███  ░░███
      ░░███ ░░███      █████   █████ █████   █████ █████  ░░█████ █████ █████  ░░█████ ░░█████████
       ░░░   ░░░      ░░░░░   ░░░░░ ░░░░░   ░░░░░ ░░░░░    ░░░░░ ░░░░░ ░░░░░    ░░░░░   ░░░░░░░░░


  Mako bundler does not support the following options:
    - ${warningKeys.join('\n    - ')}

  So this project may fail in compile-time or error in runtime, ${chalk.bold(
    'please test and release carefully',
  )}.
=====================================================================================================
      `,
      ),
    );
  }
}

async function getMakoConfig(opts) {
  const WebpackConfig = require('webpack-5-chain');
  // webpack require is handled by require hooks in bundler-webpack/src/requireHook
  const webpack = require('webpack');
  const env = process.env.NODE_ENV;
  const webpackChainConfig = new WebpackConfig();
  await opts.chainWebpack(webpackChainConfig, { env, webpack });
  if (opts.config.chainWebpack) {
    opts.config.chainWebpack(webpackChainConfig, { env, webpack });
  }
  const webpackConfig = webpackChainConfig.toConfig();
  let umd = false;
  let crossOriginLoading = false;
  let { dynamicImportToRequire } = opts.config;
  if (webpackConfig.output) {
    // handle asyncChunks config
    if (webpackConfig.output.asyncChunks === false) {
      dynamicImportToRequire = true;
    }

    // handle umd output config
    if (
      webpackConfig.output.libraryTarget === 'umd' &&
      webpackConfig.output.library
    ) {
      umd = webpackConfig.output.library;
    }

    // handle crossOriginLoading config
    if (webpackConfig.output.crossOriginLoading) {
      crossOriginLoading = webpackConfig.output.crossOriginLoading;
    }
  }

  let platform = 'browser';
  if (webpackConfig.target === 'node') platform = 'node';

  const {
    alias,
    targets,
    publicPath,
    runtimePublicPath,
    manifest,
    mdx,
    devtool,
    cjs,
    jsMinifier,
    externals,
    copy = [],
    clean,
    forkTSChecker,
    inlineCSS,
    analyze,
    sassLoader,
    mako,
  } = opts.config;

  let { codeSplitting } = opts.config;
  // TODO:
  // 暂不支持 $ 结尾，等 resolve 支持后可以把这段去掉
  Object.keys(alias).forEach((key) => {
    if (key.endsWith('$')) {
      alias[key.slice(0, -1)] = alias[key];
    }
  });

  // mako build need alias array
  const generatorAlias = Object.keys(alias).map((key) => {
    return [key, alias[key]];
  });

  const define = {};
  if (opts.config.define) {
    for (const key of Object.keys(opts.config.define)) {
      define[key] = normalizeDefineValue(opts.config.define[key]);
    }
  }

  if (process.env.SOCKET_SERVER) {
    define['process.env.SOCKET_SERVER'] = normalizeDefineValue(
      process.env.SOCKET_SERVER,
    );
  }

  let minify = jsMinifier === 'none' ? false : true;
  if (process.env.COMPRESS === 'none') {
    minify = false;
  }
  const extraBabelPlugins = [
    ...(opts.extraBabelPlugins || []),
    ...(opts.config.extraBabelPlugins || []),
  ];

  // transform babel-plugin-dynamic-import-node to dynamicImportToRequire
  dynamicImportToRequire ??= Boolean(
    extraBabelPlugins.find((p) =>
      /^(babel-plugin-)?dynamic-import-node$/.test(p),
    ),
  );

  // transform babel-plugin-import plugins to transformImport
  const transformImport = extraBabelPlugins
    .filter((p) => /^import$|babel-plugin-import/.test(p[0]))
    .map(([_, v]) => {
      const { libraryName, libraryDirectory, style, ...others } = v;

      if (Object.keys(others).length > 0) {
        throw new Error(
          `babel-plugin-import options ${Object.keys(
            others,
          )} is not supported in mako bundler`,
        );
      }

      if (typeof style === 'function') {
        throw new Error(
          'babel-plugin-import function type style is not supported in mako bundler',
        );
      }

      return { libraryName, libraryDirectory, style };
    });
  const emotion = extraBabelPlugins.some((p) => {
    return p === '@emotion' || p === '@emotion/babel-plugin';
  });
  // transform externals
  const externalsConfig = Object.entries(externals || {}).reduce(
    (ret, [k, v]) => {
      // handle [string] with script type
      if (Array.isArray(v)) {
        const [url, ...members] = v;
        ret[k] = {
          // ['antd', 'Button'] => `antd.Button`
          root: members.join('.'),
          // `script https://example.com/lib/script.js` => `https://example.com/lib/script.js`
          script: url.replace('script ', ''),
        };
      } else if (typeof v === 'string') {
        // 'window.antd' or 'window antd' => 'antd'
        ret[k] = v.replace(/^window(\s+|\.)/, '');
      } else {
        // other types except boolean has been checked before
        // so here only ignore invalid boolean type
      }
      return ret;
    },
    {},
  );
  const outputPath = path.resolve(opts.cwd, opts.config.outputPath || 'dist');
  const tsConfig = getTsConfig(opts);

  const normalizedDevtool = devtool === false ? false : 'source-map';

  let makoCodeSplittingConfig =
    codeSplitting === false
      ? false
      : codeSplitting?.jsStrategy === 'granularChunks'
        ? {
            strategy: 'granular',
            options: {
              // copy from https://github.com/umijs/umi/blob/d8f3f1fa9586a8c7cab4d3b6a9e1637a6f47e1dc/packages/preset-umi/src/features/codeSplitting/codeSplitting.ts#L60
              frameworkPackages: [
                'react-dom',
                'react',
                // 'core-js',
                // 'regenerator-runtime',
                'history',
                'react-router',
                'react-router-dom',
                'scheduler',
                // TODO
                // add renderer-react
              ],
            },
          }
        : codeSplitting?.jsStrategy === 'depPerChunk'
          ? {
              strategy: 'advanced',
              options: {
                groups: [
                  {
                    name: 'npm',
                    allowChunks: 'async',
                    test: '[\\\\/]node_modules[\\\\/]',
                    nameSuffix: 'packageName',
                    priority: -10,
                    minSize: 1,
                    minPackageSize: 1,
                  },
                  {
                    name: 'common',
                    allowChunks: 'async',
                    minChunks: 2,
                    priority: -20,
                  },
                ],
              },
            }
          : { strategy: 'auto' };

  const makoConfig = {
    entry: opts.entry,
    output: { path: outputPath, crossOriginLoading },
    resolve: {
      alias: generatorAlias,
    },
    mode: 'development',
    publicPath: runtimePublicPath ? 'runtime' : publicPath || '/',
    targets: targets || {
      chrome: 80,
    },
    manifest,
    mdx: !!mdx,
    codeSplitting: makoCodeSplittingConfig,
    devtool: normalizedDevtool,
    cjs,
    dynamicImportToRequire,
    platform,
    minify,
    define,
    autoCSSModules: true,
    umd,
    transformImport,
    externals: externalsConfig,
    clean,
    flexBugs: true,
    react: opts.react || {},
    emotion,
    inlineCSS,
    ...(opts.disableCopy ? { copy: [] } : { copy: ['public'].concat(copy) }),
    useDefineForClassFields:
      tsConfig.compilerOptions.useDefineForClassFields ?? true,
    hmr: opts.hmr || false,
    devServer: opts.devServer || false,
    forkTSChecker: !!forkTSChecker,
    less: {
      modifyVars: opts.config.lessLoader?.modifyVars || opts.config.theme,
      globalVars: opts.config.lessLoader?.globalVars,
      sourceMap: getLessSourceMapConfig(normalizedDevtool),
      math: opts.config.lessLoader?.math,
      plugins: opts.config.lessLoader?.plugins,
    },
    analyze: analyze || process.env.ANALYZE ? {} : undefined,
    sass: sassLoader,
    ...mako,
    experimental: {
      webpackSyntaxValidate: [],
      requireContext: true,
      rustPlugins: [],
      detectCircularDependence: {
        ignores: ['node_modules', '\\.umi'],
        graphviz: false,
      },
      ...mako.experimental,
    },
  };

  return makoConfig;
}

function getLessSourceMapConfig(devtool) {
  return (
    devtool && {
      sourceMapFileInline: true,
      outputSourceFiles: true,
    }
  );
}

function normalizeDefineValue(val) {
  if (!lodash.isPlainObject(val)) {
    return JSON.stringify(val);
  } else {
    return Object.keys(val).reduce((obj, key) => {
      obj[key] = normalizeDefineValue(val[key]);
      return obj;
    }, {});
  }
}

const DEFAULT_TS_CONFIG = {
  compilerOptions: {
    useDefineForClassFields: true,
  },
};

function getTsConfig(opts) {
  let root = opts.cwd;
  const tsConfigPath = path.resolve(root, 'tsconfig.json');

  if (fs.existsSync(tsConfigPath)) {
    try {
      return parseTsconfig(tsConfigPath);
    } catch (e) {
      console.error(
        'parsing tsconfig.json failed, fallback to default tsconfig\n',
        e,
      );
      return DEFAULT_TS_CONFIG;
    }
  } else {
    return DEFAULT_TS_CONFIG;
  }
}
