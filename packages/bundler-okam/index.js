const path = require('path');
const fs = require('fs');
const http = require('http');
const assert = require('assert');
const { createProxy, createHttpsServer } = require('@umijs/bundler-utils');
const { lodash, chalk } = require('@umijs/utils');

exports.build = async function (opts) {
  assert(opts, 'opts should be supplied');
  const {
    cwd,
    onBuildComplete,
    clean,
    // 以下暂不支持
    // rootDir, disableCopy, watch,
  } = opts;
  checkConfig(opts);

  if (clean) {
    const outputPath = path.join(cwd, 'dist');
    const rimraf = require('rimraf');
    rimraf.sync(outputPath);
  }

  const okamConfig = await getOkamConfig(opts);
  const mode = process.argv.includes('--dev') ? 'development' : 'production';
  okamConfig.mode = mode;
  okamConfig.manifest = true;
  okamConfig.hash = !!opts.config.hash;
  if (okamConfig.hash) {
    okamConfig.moduleIdStrategy = 'hashed';
  }

  const { build } = require('@okamjs/okam');
  await build(cwd, okamConfig, false);

  // TODO: use stats
  const manifest = JSON.parse(
    fs.readFileSync(
      path.join(
        cwd,
        'dist',
        okamConfig.manifestConfig?.fileName || 'asset-manifest.json',
      ),
    ),
  );
  const assets = Object.keys(manifest)
    .filter((key) => !key.endsWith('.map'))
    .reduce((obj, key) => {
      obj[manifest[key]] = 1;
      return obj;
    }, {});
  const stats = {
    compilation: { assets },
    hasErrors: () => false,
  };
  onBuildComplete({
    err: null,
    stats,
  });
  return stats;
};

exports.dev = async function (opts) {
  assert(opts, 'opts should be supplied');
  checkConfig(opts.config);
  const express = require('express');
  const app = express();
  // cros
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
  // serve dist files
  app.use(express.static(path.join(opts.cwd, 'dist')));
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
  const port = opts.port || 8000;
  server.listen(port, () => {
    const protocol = opts.config.https ? 'https:' : 'http:';
    const banner = getDevBanner(protocol, opts.host, port, opts.ip);
    console.log(banner);
  });
  // okam dev
  const { build } = require('@okamjs/okam');
  const okamConfig = await getOkamConfig(opts);
  okamConfig.hmr = true;
  okamConfig.hmr_port = String(opts.port + 1);
  okamConfig.hmr_host = opts.host;
  await build(opts.cwd, okamConfig, true);
};

function getDevBanner(protocol, host, port, ip) {
  const chalk = require('chalk');
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
    // 暂不支持 legacy
    'legacy',
    // 暂不支持多 entry
    'mpa',
  ];

  // 处理构建不支持的配置项
  unsupportedKeys.forEach((key) => {
    assert(!opts.config[key], `${key} is not supported in okam bundler`);
  });

  // 暂不支持 { from, to } 格式
  const { copy } = opts.config;
  if (copy) {
    for (const item of copy) {
      assert(
        typeof item === 'string',
        `copy config item must be string in okam bundler, but got ${item}`,
      );
    }
  }

  // 不支持数组 externals
  if (Array.isArray(opts.config.externals)) {
    throw new Error('externals array is not supported in okam bundler');
  }

  // 不支持但对构建影响不明确的配置项，会统一警告
  const riskyKeys = [
    'config.autoprefixer',
    'config.analyze',
    'config.cssPublicPath',
    'config.cssLoader',
    'config.cssLoaderModules',
    'config.classPropertiesLoose',
    'config.extraPostCSSPlugins',
    'config.forkTSChecker',
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
      if (!/^import$|babel-plugin-import/.test(p[0])) {
        warningKeys.push('extraBabelPlugins');
        return true;
      }
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


  Okam bundler does not support the following options:
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

async function getOkamConfig(opts) {
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
  let umd = 'none';
  if (
    webpackConfig.output &&
    webpackConfig.output.libraryTarget === 'umd' &&
    webpackConfig.output.library
  ) {
    umd = webpackConfig.output.library;
  }

  const {
    alias,
    targets,
    publicPath,
    runtimePublicPath,
    manifest,
    mdx,
    theme,
    lessLoader,
    codeSplitting,
    devtool,
    jsMinifier,
    externals,
  } = opts.config;
  const outputPath = path.join(opts.cwd, 'dist');
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
        define[key.replace(/^process\.env\./, '')] = opts.config.define[key];
      } else {
        define[key] = normalizeDefineValue(opts.config.define[key]);
      }
    }
  }
  let minify = jsMinifier === 'none' ? false : true;
  if (process.env.COMPRESS === 'none') {
    minify = false;
  }
  // transform babel-plugin-import plugins to transformImport
  const transformImport = [
    ...(opts.extraBabelPlugins || []),
    ...(opts.config.extraBabelPlugins || []),
  ]
    .filter((p) => /^import$|babel-plugin-import/.test(p[0]))
    .map(([_, v]) => {
      const { libraryName, libraryDirectory, style, ...others } = v;

      if (Object.keys(others).length > 0) {
        throw new Error(
          `babel-plugin-import options ${Object.keys(
            others,
          )} is not supported in okam bundler`,
        );
      }

      if (typeof style === 'function') {
        throw new Error(
          `babel-plugin-import function type style is not supported in okam bundler`,
        );
      }

      return { libraryName, libraryDirectory, style };
    });
  const okamConfig = {
    entry: opts.entry,
    output: { path: outputPath },
    resolve: {
      alias: {
        ...alias,
        '@swc/helpers': path.dirname(
          require.resolve('@swc/helpers/package.json'),
        ),
        'node-libs-browser-okam': path.dirname(
          require.resolve('node-libs-browser-okam/package.json'),
        ),
        'react-refresh': path.dirname(
          require.resolve('react-refresh/package.json'),
        ),
        'react-error-overlay': path.dirname(
          require.resolve('react-error-overlay/package.json'),
        ),
      },
    },
    mode: 'development',
    publicPath: runtimePublicPath ? 'runtime' : publicPath || '/',
    targets: targets || {
      chrome: 80,
    },
    manifest: !!manifest,
    manifestConfig: manifest || {},
    mdx: !!mdx,
    codeSplitting: codeSplitting === false ? 'none' : 'auto',
    devtool: devtool === false ? 'none' : 'source-map',
    less: {
      theme: {
        ...theme,
        ...lessLoader?.modifyVars,
      },
      javascriptEnabled: lessLoader?.javascriptEnabled,
      lesscPath: path.join(
        path.dirname(require.resolve('less/package.json')),
        'bin/lessc',
      ),
    },
    minify,
    define,
    autoCSSModules: true,
    umd,
    transformImport,
    externals,
  };

  if (process.env.DUMP_MAKO_CONFIG) {
    const configFile = path.join(process.cwd(), 'mako.config.json');
    fs.writeFileSync(configFile, JSON.stringify(okamConfig, null, 2));
  }

  return okamConfig;
}

function normalizeDefineValue(val) {
  if (!isPlainObject(val)) {
    return JSON.stringify(val);
  } else {
    return Object.keys(val).reduce((obj, key) => {
      obj[key] = normalizeDefineValue(val[key]);
      return obj;
    }, {});
  }
}

function isPlainObject(obj) {
  return Object.prototype.toString.call(obj) === '[object Object]';
}
