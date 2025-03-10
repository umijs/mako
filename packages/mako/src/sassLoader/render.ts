import { type Options } from 'sass';
import { RunLoadersOptions, runLoaders } from '../runLoaders';

async function render(param: {
  filename: string;
  opts: Options<'async'> & { resources: string[] };
  extOpts: RunLoadersOptions;
}): Promise<{ content: string; type: 'css' }> {
  const options = { style: 'compressed', ...param.opts };
  const extOpts = param.extOpts;

  const content = await runLoaders({
    alias: extOpts.alias,
    root: extOpts.root,
    resource: param.filename,
    loaders: [
      {
        loader: require.resolve('sass-loader'),
        options,
      },
    ],
  })
    .then((result) => {
      let source: string = '';
      if (result.result) {
        const buf = result.result[0];
        if (Buffer.isBuffer(buf)) {
          source = buf.toString('utf-8');
        } else {
          source = buf ?? '';
        }
      }
      return source;
    })
    .catch((err) => {
      throw new Error(err.toString());
    });

  return { content: content, type: 'css' };
}

export { render };
