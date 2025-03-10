import { LessLoaderOpts } from '.';
import { RunLoadersOptions, runLoaders } from '../../runLoaders';

module.exports = async function render(param: {
  filename: string;
  opts: LessLoaderOpts;
  extOpts: RunLoadersOptions;
}): Promise<{ content: string; type: 'css' }> {
  const { modifyVars, globalVars, math, sourceMap, plugins } = param.opts;
  const extOpts = param.extOpts;

  const pluginInstances: Less.Plugin[] | undefined = plugins?.map((p) => {
    if (Array.isArray(p)) {
      const pluginModule = require(p[0]);
      const PluginClass = pluginModule.default || pluginModule;
      return new PluginClass(p[1]);
    } else {
      return require(p);
    }
  });

  const content = await runLoaders({
    alias: extOpts.alias,
    root: extOpts.root,
    resource: param.filename,
    loaders: [
      {
        loader: require.resolve('less-loader'),
        options: {
          lessOptions: {
            filename: param.filename,
            javascriptEnabled: true,
            math,
            plugins: pluginInstances,
            modifyVars,
            globalVars,
            rewriteUrls: 'all',
            sourceMap,
          },
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
          source = buf ?? '';
        }
      }
      return source;
    })
    .catch((err) => {
      throw new Error(err.toString());
    });

  return { content: content, type: 'css' };
};
