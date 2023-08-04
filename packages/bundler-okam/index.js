const path = require('path');
const assert = require('assert');

exports.build = async function (opts) {
  assert(opts, 'opts should be supplied');
  const {
    cwd,
    onBuildComplete,
    clean,
    // 以下暂不支持
    // rootDir, disableCopy, watch,
  } = opts;
  checkConfig(opts.config);

  if (clean) {
    const outputPath = path.join(cwd, 'dist');
    const rimraf = require('rimraf');
    rimraf.sync(outputPath);
  }

  const okamConfig = getOkamConfig(opts);
  const mode = process.argv.includes('--dev') ? 'development' : 'production';
  okamConfig.mode = mode;

  const { build } = require('@okamjs/okam');
  await build(cwd, okamConfig, false);

  // FIXME: mock stats
  const stats = {
    compilation: { assets: { 'umi.js': 'umi.js' } },
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
  // TODO: proxy
  // opts.config.proxy
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
  // TODO: https
  // opts.config.https
  const server = require('http').createServer(app);
  const port = opts.port || 8000;
  server.listen(port, () => {
    const protocol = opts.config.https ? 'https:' : 'http:';
    const banner = getDevBanner(protocol, opts.host, port, opts.ip);
    console.log(banner);
  });
  // okam dev
  const { build } = require('@okamjs/okam');
  const okamConfig = getOkamConfig(opts);
  okamConfig.hmr = true;
  // TODO: detect port
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

function checkConfig(config) {
  assert(!config.mfsu, 'mfsu is not supported in okam bundler');
}

function getOkamConfig(opts) {
  const { alias, targets, publicPath, runtimePublicPath, manifest } =
    opts.config;
  const outputPath = path.join(opts.cwd, 'dist');
  // TODO:
  // 暂不支持 $ 结尾，等 resolve 支持后可以把这段去掉
  Object.keys(alias).forEach((key) => {
    if (key.endsWith('$')) {
      alias[key.slice(0, -1)] = alias[key];
    }
  });
  return {
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
    public_path: runtimePublicPath ? 'runtime' : publicPath || '/',
    targets: targets || {
      chrome: 80,
    },
    manifest: !!manifest,
  };
}
