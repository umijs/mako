import fs from 'fs';
import * as EnhancedResolve from 'enhanced-resolve';
import { LessLoaderOpts } from '.';
import { runLoaders } from '../loaderContext';

module.exports = async function render(param: {
  filename: string;
  opts: LessLoaderOpts;
  root: string;
}): Promise<{ content: string; type: 'css' }> {
  const { modifyVars, globalVars, math, sourceMap, plugins } = param.opts;

  const pluginInstances: Less.Plugin[] | undefined = plugins?.map((p) => {
    if (Array.isArray(p)) {
      const pluginModule = require(p[0]);
      const PluginClass = pluginModule.default || pluginModule;
      return new PluginClass(p[1]);
    } else {
      return require(p);
    }
  });

  const resolver = EnhancedResolve.ResolverFactory.createResolver({
    fileSystem: new EnhancedResolve.CachedInputFileSystem(fs, 60000),
    conditionNames: ['less', 'style', '...'],
    mainFields: ['less', 'style', 'main', '...'],
    mainFiles: ['index', '...'],
    extensions: ['.less', '.css'],
    preferRelative: true,
  });

  const content = await runLoaders({
    root: param.root,
    resource: param.filename,
    resolver,
    loaders: [
      {
        loader: require.resolve('less-loader'),
        options: {
          filename: param.filename,
          javascriptEnabled: true,
          math,
          plugins: pluginInstances,
          modifyVars,
          globalVars,
          sourceMap,
          rewriteUrls: 'all',
        },
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
};
