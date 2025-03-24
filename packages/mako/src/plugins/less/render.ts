import { LessLoaderOpts } from '.';
import { RunLoadersOptions, runLoaders } from '../../runLoaders';

module.exports = async function render(param: {
  filename: string;
  opts: LessLoaderOpts & { postcss?: boolean };
  extOpts: RunLoadersOptions;
}) {
  const {
    modifyVars,
    globalVars,
    math,
    sourceMap,
    plugins,
    postcss: postcssOptions,
  } = param.opts;
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

  return runLoaders({
    alias: extOpts.alias,
    root: extOpts.root,
    resource: param.filename,
    loaders: [
      postcssOptions && {
        loader: require.resolve('postcss-loader'),
      },
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
    ].filter(Boolean),
  })
    .then((result) => result)
    .catch((err) => {
      throw new Error(err.toString());
    });
};
