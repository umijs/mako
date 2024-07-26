import { type Options } from 'sass';

module.exports = async function render(param: {
  filename: string;
  opts: Omit<Options<'async'>, 'functions'>;
}): Promise<{ content: string; type: 'css' }> {
  let sass;
  try {
    sass = require('sass');
  } catch (err) {
    console.error(
      'The "sass" package is not installed. Please run "npm install sass" to install it.',
    );
    return { content: '', type: 'css' };
  }
  const result = await sass
    .compileAsync(param.filename, { style: 'compressed', ...param.opts })
    .catch((err) => {
      throw new Error(err.toString());
    });
  return { content: result.css, type: 'css' };
};
