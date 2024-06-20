const { build } = require('@alipay/umi-bundler-okam');

build({
  cwd: __dirname,
  entry: {
    index: './index.tsx',
  },
  config: {
    alias: [],
  },
  onBuildComplete() {
    console.log('build finished');
  },
});
