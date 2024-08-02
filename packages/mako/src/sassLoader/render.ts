import { type Options } from 'sass';

async function render(param: {
  filename: string;
  opts: Options<'async'> & { resources: string[] };
}): Promise<{ content: string; type: 'css' }> {
  let sass;
  try {
    sass = require('sass');
  } catch (err) {
    throw new Error(
      'The "sass" package is not installed. Please run "npm install sass" to install it.',
    );
  }
  const result = await sass
    .compileAsync(param.filename, { style: 'compressed', ...param.opts })
    .catch((err: any) => {
      throw new Error(err.toString());
    });
  return { content: result.css, type: 'css' };
}

export { render };
