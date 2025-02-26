import { type Options } from 'sass';
import { createImporter } from './importer';

async function render(param: {
  filename: string;
  opts: Options<'async'> & { resources: string[] };
}): Promise<{ content: string; type: 'css' }> {
  let sass: any;
  try {
    sass = require('sass');
  } catch (err) {
    throw new Error(
      'The "sass" package is not installed. Please run "npm install sass" to install it.',
    );
  }

  const options = { style: 'compressed', ...param.opts };
  options.importers = options.importers || [];
  options.importers.push(createImporter(param.filename, sass));

  const result = await sass
    .compileAsync(param.filename, options)
    .catch((err: any) => {
      throw new Error(err.toString());
    });
  return { content: result.css, type: 'css' };
}

export { render };
