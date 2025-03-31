import { RunLoadersOptions, runLoaders } from '../../runLoaders';

module.exports = async function render(param: {
  filename: string;
  extOpts: RunLoadersOptions;
}) {
  const extOpts = param.extOpts;

  return runLoaders({
    alias: extOpts.alias,
    root: extOpts.root,
    resource: param.filename,
    loaders: [
      {
        loader: require.resolve('postcss-loader'),
      },
    ],
  })
    .then((result) => result)
    .catch((err) => {
      throw new Error(err.toString());
    });
};
