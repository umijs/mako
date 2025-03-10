import { type Options } from 'sass';
import { runLoaders } from '../loaderContext';

async function render(param: {
  filename: string;
  opts: Options<'async'> & { resources: string[] };
  root: string;
}): Promise<{ content: string; type: 'css' }> {
  const options = { style: 'compressed', ...param.opts };

  const content = await runLoaders({
    root: param.root,
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
          source = buf || '';
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
