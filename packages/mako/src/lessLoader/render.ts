import fs from 'fs';
import less from 'less';
import { LessLoaderOpts } from '.';

module.exports = async function render(param: {
  filename: string;
  opts: LessLoaderOpts;
}): Promise<{ content: string; type: 'css' }> {
  const { modifyVars, globalVars, math, sourceMap, plugins } = param.opts;
  const input = fs.readFileSync(param.filename, 'utf-8');

  const pluginInstances: Less.Plugin[] | undefined = plugins?.map((p) => {
    if (Array.isArray(p)) {
      const pluginModule = require(p[0]);
      const PluginClass = pluginModule.default || pluginModule;
      return new PluginClass(p[1]);
    } else {
      return require(p);
    }
  });

  const result = await less
    .render(input, {
      filename: param.filename,
      javascriptEnabled: true,
      math,
      plugins: pluginInstances,
      modifyVars,
      globalVars,
      sourceMap,
      rewriteUrls: 'all',
    } as unknown as Less.Options)
    .catch((err) => {
      throw new Error(err.toString());
    });

  return { content: result.css, type: 'css' };
};
