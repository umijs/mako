const bundler = require('../../');
const noop = () => {};

bundler
  .build({
    cwd: __dirname,
    config: {
      entry: {
        index: 'index.ts',
      },
      alias: [],
      jsMinifier: 'none',
      hash: false,
      targets: {
        chrome: 40,
      },
    },
    onBuildComplete: noop,
    chainWebpack: noop,
    watch: false,
  })
  .then(
    () => console.log('Build completed'),
    (e) => console.log(e),
  );
