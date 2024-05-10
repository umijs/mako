const path = require('path');
const fs = require('fs');
const http = require('http');
const assert = require('assert');
const { createProxy, createHttpsServer } = require('@umijs/bundler-utils');
const lodash = require('lodash');
const chalk = require('chalk');
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
  makoConfig.stats = true;
  makoConfig.mode = 'production';
  makoConfig.hash = !!opts.config.hash;
  if (makoConfig.hash) {
    makoConfig.moduleIdStrategy = 'hashed';
  }

  const { build } = require('@umijs/mako');
  try {
    await build({
      root: cwd,
      config: makoConfig,
      less: {
        modifyVars: opts.config.lessLoader?.modifyVars || opts.config.theme,
        sourceMap: getLessSourceMapConfig(makoConfig.devtool),
        math: opts.config.lessLoader?.math,
        pluginsForMako: opts.config.lessLoader?.pluginsForMako,
      },
      forkTSChecker: makoConfig.forkTSChecker,
      watch: false,
    });
  } catch (e) {
    console.error(e.message);
    opts.onBuildError?.(e);
    const err = new Error('Build with mako failed.');
    err.stack = null;
    throw err;
  }
  const outputPath = path.resolve(opts.cwd, opts.config.outputPath || 'dist');

  const statsJsonPath = path.join(outputPath, 'stats.json');
  const statsJson = JSON.parse(fs.readFileSync(statsJsonPath, 'utf-8'));

  // remove stats.json file if user did not enable it
  if (originStats !== true) fs.rmSync(statsJsonPath);

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

  const outputPath = path.resolve(opts.cwd, opts.config.outputPath || 'dist');

  // serve dist files
  app.use(express.static(outputPath));
  // proxy
  if (opts.config.proxy) {
    createProxy(opts.config.proxy, app);
  }
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
  makoConfig.hmr = { port: hmrPort, host: opts.host };
  const cwd = opts.cwd;
  try {
    await build({
      root: cwd,
      config: makoConfig,
      less: {
        modifyVars: opts.config.lessLoader?.modifyVars || opts.config.theme,
        sourceMap: getLessSourceMapConfig(makoConfig.devtool),
        math: opts.config.lessLoader?.math,
        pluginsForMako: opts.config.lessLoader?.pluginsForMako,
      },
      forkTSChecker: makoConfig.forkTSChecker,
      hooks: {
        generateEnd: (args) => {
          opts.onDevCompileDone(args);
        },
      },
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
    'config.analyze',
    'config.cssPublicPath',
    'config.cssLoader',
    'config.cssLoaderModules',
    'config.classPropertiesLoose',
    'config.extraPostCSSPlugins',
    'config.postcssLoader',
    'config.sassLoader',
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
        'math',
        'pluginsForMako',
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
      if (!isImportPlugin && !isEmotionPlugin) {
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
  if (
    webpackConfig.output &&
    webpackConfig.output.libraryTarget === 'umd' &&
    webpackConfig.output.library
  ) {
    umd = webpackConfig.output.library;
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
    codeSplitting,
    devtool,
    cjs,
    dynamicImportToRequire,
    jsMinifier,
    externals,
    copy = [],
    clean,
    forkTSChecker,
  } = opts.config;
  // TODO:
  // 暂不支持 $ 结尾，等 resolve 支持后可以把这段去掉
  Object.keys(alias).forEach((key) => {
    if (key.endsWith('$')) {
      alias[key.slice(0, -1)] = alias[key];
    }
  });
  const define = {};
  if (opts.config.define) {
    for (const key of Object.keys(opts.config.define)) {
      // mako 的 define 会先去判断 process.env.xxx，再去判断 xxx
      // 这里传 process.env.xxx 反而不会生效
      // TODO: 待 mako 改成和 umi/webpack 的方式一致之后，可以把这段去掉
      if (key.startsWith('process.env.')) {
        define[key.replace(/^process\.env\./, '')] = normalizeDefineValue(
          opts.config.define[key],
        );
      } else {
        define[key] = normalizeDefineValue(opts.config.define[key]);
      }
    }
  }

  if (process.env.SOCKET_SERVER) {
    define.SOCKET_SERVER = normalizeDefineValue(process.env.SOCKET_SERVER);
  }

  let minify = jsMinifier === 'none' ? false : true;
  if (process.env.COMPRESS === 'none') {
    minify = false;
  }
  // transform babel-plugin-import plugins to transformImport
  const extraBabelPlugins = [
    ...(opts.extraBabelPlugins || []),
    ...(opts.config.extraBabelPlugins || []),
  ];
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

  const makoConfig = {
    entry: opts.entry,
    output: { path: outputPath },
    resolve: {
      alias,
    },
    mode: 'development',
    publicPath: runtimePublicPath ? 'runtime' : publicPath || '/',
    targets: targets || {
      chrome: 80,
    },
    manifest,
    mdx: !!mdx,
    codeSplitting: codeSplitting === false ? false : 'auto',
    devtool: devtool === false ? false : 'source-map',
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
    forkTSChecker: !!forkTSChecker,
    ...(opts.disableCopy ? { copy: [] } : { copy: ['public'].concat(copy) }),
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
