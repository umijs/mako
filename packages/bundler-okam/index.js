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
  console.log('opts', opts);

  if (clean) {
    rimraf.sync(outputPath);
  }

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
    externals: { stream: 'stream' },
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
