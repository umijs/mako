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
        'node-libs-browser-okam': path.dirname(
          require.resolve('node-libs-browser-okam/package.json'),
        ),
      },
    },
    mode,
    public_path: runtimePublicPath ? 'runtime' : publicPath || '/',
    targets: targets || {
      chrome: 80,
    },
  };

  const { build } = require('@alipay/okam');
  build(cwd, config);

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

exports.dev = async function () {
  throw new Error('not implement yet');
};
