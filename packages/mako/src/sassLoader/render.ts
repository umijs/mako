import sass, { type Options } from 'sass';

module.exports = async function render(param: {
  filename: string;
  opts: Options<'async'>;
}): Promise<{ content: string; type: 'css' }> {
  const result = await sass
    .compileAsync(param.filename, { style: 'compressed', ...param.opts })
    .catch((err) => {
      throw new Error(err.toString());
    });
  return { content: result.css, type: 'css' };
};
