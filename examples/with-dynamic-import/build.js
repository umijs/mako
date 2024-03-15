const { build } = require('@alipay/umi-bundler-okam');

build({
  cwd: __dirname,
  entry: {
    index: './index.tsx',
  },
  chainWebpack: () => {},
  config: {
    alias: {},
    externals: {},
    tsChecker: true,
  },
  onBuildComplete() {
    console.log('build finished');
  },
});
