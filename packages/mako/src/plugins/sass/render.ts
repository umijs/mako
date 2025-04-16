import { type Options } from 'sass';
import {
  RunLoaderResult,
  RunLoadersOptions,
  runLoaders,
} from '../../runLoaders';

export async function renderSass(param: {
  filename: string;
  opts: Options<'async'>;
  extOpts: RunLoadersOptions;
}): Promise<RunLoaderResult & { missingDependencies: string[] }> {
  const options = { style: 'compressed', ...param.opts };
  const extOpts = param.extOpts;

  return runLoaders({
    alias: extOpts.alias,
    root: extOpts.root,
    resource: param.filename,
    loaders: [
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
