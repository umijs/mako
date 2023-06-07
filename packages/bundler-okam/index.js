const rimraf = require('rimraf');
const path = require('path');

exports.build = async function (opts) {
  const {
    cwd,
    entry,
    onBuildComplete,
    clean,
    // 以下暂不支持
    // rootDir, disableCopy, watch,
  } = opts;
  const { alias, targets, publicPath, runtimePublicPath } = opts.config;

  const outputPath = path.join(cwd, 'dist');

  if (clean) {
    rimraf.sync(outputPath);
  }

  // TODO:
  // 暂不支持 $ 结尾，等 resolve 支持后可以把这段去掉
  Object.keys(alias).forEach((key) => {
    if (key.endsWith('$')) {
      alias[key.slice(0, -1)] = alias[key];
    }
  });

  // TODO:
  // 现在没有走内置的 config，这里先手动加上 node 补丁的 externals
  const externals = [
    'assert',
    'assert/strict',
    'async_hooks',
    'buffer',
    'child_process',
    'cluster',
    'console',
    'constants',
    'crypto',
    'dgram',
    'diagnostics_channel',
    'dns',
    'dns/promises',
    'domain',
    'events',
    'fs',
    'fs/promises',
    'http',
    'http2',
    'https',
    'inspector',
    'inspector/promises',
    'module',
    'net',
    'os',
    'path',
    'path/posix',
    'path/win32',
    'perf_hooks',
    'process',
    'punycode',
    'querystring',
    'readline',
    'readline/promises',
    'repl',
    'stream',
    'stream/consumers',
    'stream/promises',
    'stream/web',
    'string_decoder',
    'sys',
    'timers',
    'timers/promises',
    'tls',
    'trace_events',
    'tty',
    'url',
    'util',
    'util/types',
    'v8',
    'vm',
    'wasi',
    'worker_threads',
    'zlib',
  ].reduce((memo, key) => {
    memo[key] = key;
    return memo;
  }, {});

  const mode = process.argv.includes('--dev') ? 'development' : 'production';
  const config = {
    entry,
    output: { path: outputPath },
    resolve: {
      alias: {
        ...alias,
        '@swc/helpers': path.dirname(
          require.resolve('@swc/helpers/package.json'),
        ),
      },
      extensions: ['.js', '.jsx', '.ts', '.tsx', '.mjs', '.cjs', '.json'],
    },
    mode,
    sourcemap: true,
    externals,
    copy: ['public'],
    data_url_limit: 10000,
    public_path: runtimePublicPath ? 'runtime' : publicPath || '/',
    devtool: 'source-map',
    targets: targets || {
      chrome: 80,
    },
  };

  const { build } = require('@alipay/okam');
  build(cwd, config);

  onBuildComplete({
    err: null,
  });

  const stats = { compilation: { assets: { 'umi.js': 'umi.js' } } };
  return stats;
};

exports.dev = async function () {
  throw new Error('not implement yet');
};
