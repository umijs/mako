import { LessLoaderOpts } from '.';
import { RunLoadersOptions, runLoaders } from '../../runLoaders';
import LessImportPlugin from './less-import-plugin';

module.exports = async function render(param: {
  filename: string;
  opts: LessLoaderOpts;
  extOpts: RunLoadersOptions;
}) {
  const { modifyVars, globalVars, math, sourceMap, plugins } = param.opts;
  const extOpts = param.extOpts;

  const pluginInstances: Less.Plugin[] | undefined = (plugins || []).map(
    (p) => {
      if (Array.isArray(p)) {
        const pluginModule = require(p[0]);
        const PluginClass = pluginModule.default || pluginModule;
        return new PluginClass(p[1]);
      } else {
        return require(p);
      }
    },
  );

  pluginInstances.unshift(new LessImportPlugin());

  return runLoaders({
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
    .then((result) => result)
    .catch((err) => {
      throw new Error(err.toString());
    });
};
