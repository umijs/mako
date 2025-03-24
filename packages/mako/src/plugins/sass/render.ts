import { type Options } from 'sass';
import { RunLoadersOptions, runLoaders } from '../../runLoaders';

async function render(param: {
  filename: string;
  opts: Options<'async'> & { resources: string[]; postcss?: boolean };
  extOpts: RunLoadersOptions;
}) {
  const { postcss: postcssOptions, ...rest } = param.opts;
  const options = { style: 'compressed', ...rest };
  const extOpts = param.extOpts;

  return runLoaders({
    alias: extOpts.alias,
    root: extOpts.root,
    resource: param.filename,
    loaders: [
      postcssOptions && {
        loader: require.resolve('postcss-loader'),
      },
      {
        loader: require.resolve('sass-loader'),
        options: {
          sassOptions: options,
        },
      },
    ].filter(Boolean),
  })
    .then((result) => result)
    .catch((err) => {
      throw new Error(err.toString());
    });
}

module.exports = render;
