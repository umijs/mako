import { type Options } from 'sass';
import { RunLoadersOptions, runLoaders } from '../../runLoaders';

async function render(param: {
  filename: string;
  opts: Options<'async'> & { resources: string[] };
  extOpts: RunLoadersOptions;
  postLoaders?: Array<{
    loader: string;
    options?: Record<string, unknown>;
  }>;
}) {
  const options = { style: 'compressed', ...param.opts };
  const extOpts = param.extOpts;

  return runLoaders({
    alias: extOpts.alias,
    root: extOpts.root,
    resource: param.filename,
    loaders: [
      ...(param.postLoaders || []),
      {
        loader: require.resolve('sass-loader'),
        options: {
          sassOptions: options,
        },
      },
    ],
  })
    .then((result) => result)
    .catch((err) => {
      throw new Error(err.toString());
    });
}

module.exports = render;
