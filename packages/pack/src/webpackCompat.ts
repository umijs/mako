import type webpack from "webpack";
import {
  BundleOptions,
  ConfigComplete,
  TurbopackRuleConfigItem,
} from "./types";

export type WebpackConfig = Pick<
  webpack.Configuration,
  | "name"
  | "entry"
  | "mode"
  | "module"
  | "resolve"
  | "externals"
  | "output"
  | "target"
  | "devtool"
  | "optimization"
  | "plugins"
  | "stats"
> & {
  compatMode: true;
};

export function compatOptionsFromWebpack(
  webpackConfig: WebpackConfig,
): BundleOptions {
  const {
    entry,
    mode,
    module,
    resolve,
    externals,
    output,
    target,
    devtool,
    optimization,
    plugins,
    stats,
  } = webpackConfig;

  return {
    config: {
      entry: compatEntry(entry),
      mode: compatMode(mode),
      module: compatModule(module),
      resolve: compatResolve(resolve),
      externals: compatExternals(externals),
      output: compatOutput(output),
      target: compatTarget(target),
      sourceMaps: compatSourceMaps(devtool),
      optimization: compatOptimization(optimization),
      define: compatFromWebpackPlugin(plugins, compatDefine),
      stats: compatStats(stats),
    },
    buildId: webpackConfig.name,
  };
}

function compatMode(
  webpackMode: webpack.Configuration["mode"],
): "development" | "production" | undefined {
  return webpackMode
    ? webpackMode === "none"
      ? "production"
      : webpackMode
    : "production";
}

function compatEntry(webpackEntry: WebpackConfig["entry"]) {
  const entry: ConfigComplete["entry"] = [];

  switch (typeof webpackEntry) {
    case "string":
      entry.push({ import: webpackEntry });
      break;
    case "object":
      if (Array.isArray(webpackEntry)) {
        webpackEntry.forEach((e) =>
          entry.push({
            import: e,
          }),
        );
      } else {
        Object.entries(webpackEntry).forEach(([k, v]) => {
          switch (typeof v) {
            case "string":
              entry.push({ name: k, import: v });
              break;
            case "object":
              if (!Array.isArray(v)) {
                switch (typeof v.import) {
                  case "string":
                    entry.push({
                      name: k,
                      import: v.import,
                      library:
                        v.library?.type === "umd"
                          ? {
                              name:
                                typeof v.library.name === "string"
                                  ? v.library.name
                                  : undefined,
                              export:
                                typeof v.library.export === "string"
                                  ? [v.library.export]
                                  : v.library.export,
                            }
                          : undefined,
                    });
                    break;

                  default:
                    break;
                }
              } else {
                throw "multi entry items for one entry not supported yet";
              }
              break;
            default:
              throw "non string and non object entry path not supported yet";
          }
        });
      }
      break;
    case "function":
      throw "functional entry not supported yet";
    default:
      throw "entry config not compatible now";
  }

  return entry;
}

type MaybeWebpackPluginInstance = undefined | webpack.WebpackPluginInstance;

type WebpackPluginOptionsPicker<R> = ((p: MaybeWebpackPluginInstance) => R) & {
  pluginName: string;
};

function compatFromWebpackPlugin<R>(
  webpackPlugins: webpack.Configuration["plugins"],
  picker: WebpackPluginOptionsPicker<R>,
): R {
  const plugin = webpackPlugins?.find(
    (p) =>
      p && typeof p === "object" && p.constructor.name === picker.pluginName,
  ) as MaybeWebpackPluginInstance;
  return picker(plugin);
}

compatDefine.pluginName = "DefinePlugin";
function compatDefine(maybeWebpackPluginInstance: MaybeWebpackPluginInstance) {
  return maybeWebpackPluginInstance?.definitions;
}

function compatExternals(webpackExternal: webpack.Configuration["externals"]) {
  let externals: ConfigComplete["externals"] = {};
  switch (typeof webpackExternal) {
    case "string":
      externals[webpackExternal] = webpackExternal;
      break;
    case "object":
      if (webpackExternal instanceof RegExp) {
        throw "regex enternal not supported yet";
      } else if (Array.isArray(webpackExternal)) {
        webpackExternal.forEach((k) => {
          switch (typeof k) {
            case "string":
              externals[k] = k;
              break;
            default:
              throw "non string external item not supported yet";
          }
        });
      } else {
        if ("byLayer" in webpackExternal) {
          throw "by layer external item not supported yet";
        }
        Object.entries(webpackExternal).forEach(([k, v]) => {
          switch (typeof v) {
            case "string":
              externals[k] = v;
              break;
            default:
              throw "non string external item not supported yet";
          }
        });
      }
      break;
    case "function":
      throw "functional external not supported yet";
    default:
      break;
  }

  return externals;
}

function compatModule(
  webpackModule: webpack.Configuration["module"],
): ConfigComplete["module"] {
  if (!Array.isArray(webpackModule?.rules)) {
    return;
  }
  const moduleRules = {
    rules: webpackModule.rules.reduce(
      (acc, cur) => {
        switch (typeof cur) {
          case "object":
            if (cur) {
              let condition = cur.test?.toString().match(/(\.\w+)/)?.[1];
              if (condition) {
                Object.assign(acc, {
                  ["*" + condition]: <TurbopackRuleConfigItem>{
                    loaders:
                      typeof cur.use === "string"
                        ? [cur.use]
                        : typeof cur?.use === "object"
                          ? Array.isArray(cur.use)
                            ? cur.use.map((use) =>
                                typeof use === "string"
                                  ? { loader: use, options: {} }
                                  : {
                                      loader: (<any>use).loader,
                                      options: (<any>use).options || {},
                                    },
                              )
                            : [
                                {
                                  loader: cur.loader!,
                                  options: cur.options || {},
                                },
                              ]
                          : [],
                    as: "*.js",
                  },
                });
              }
            }
            break;
          default:
            break;
        }

        return acc;
      },
      {} as Record<string, TurbopackRuleConfigItem>,
    ),
  };

  return moduleRules;
}

function compatResolve(
  webpackResolve: webpack.Configuration["resolve"],
): ConfigComplete["resolve"] {
  if (!webpackResolve) {
    return;
  }
  const { alias, extensions } = webpackResolve;
  return {
    alias: alias
      ? Array.isArray(alias)
        ? alias.reduce(
            (acc, cur) => Object.assign(acc, { [cur.name]: cur.alias }),
            {},
          )
        : Object.entries(alias).reduce((acc, [k, v]) => {
            if (typeof v === "string") {
              Object.assign(acc, { [k]: v });
            } else {
              throw "non string alias value not supported yet";
            }
            return acc;
          }, {})
      : undefined,
    extensions,
  };
}

function compatOutput(
  webpackOutput: webpack.Configuration["output"],
): ConfigComplete["output"] {
  if (webpackOutput?.filename && typeof webpackOutput.filename !== "string") {
    throw "non string output filename not supported yet";
  }
  if (
    webpackOutput?.chunkFilename &&
    typeof webpackOutput.chunkFilename !== "string"
  ) {
    throw "non string output chunkFilename not supported yet";
  }
  return {
    path: webpackOutput?.path,
    filename: webpackOutput?.filename as string | undefined,
    chunkFilename: webpackOutput?.chunkFilename as string | undefined,
    clean: !!webpackOutput?.clean,
  };
}

function compatTarget(
  webpackTarget: webpack.Configuration["target"],
): ConfigComplete["target"] {
  return webpackTarget
    ? Array.isArray(webpackTarget)
      ? webpackTarget.join(" ")
      : webpackTarget
    : undefined;
}

function compatSourceMaps(
  webpackSourceMaps: webpack.Configuration["devtool"],
): ConfigComplete["sourceMaps"] {
  return !!webpackSourceMaps;
}

function compatOptimization(
  webpackOptimization: webpack.Configuration["optimization"],
): ConfigComplete["optimization"] {
  if (!webpackOptimization) {
    return;
  }
  const {
    moduleIds,
    minimize,
    // TODO: concatenateModules to be supported, need to upgrade to next.js@15.4
  } = webpackOptimization;
  return {
    moduleIds:
      moduleIds === "named"
        ? "named"
        : moduleIds === "deterministic"
          ? "deterministic"
          : undefined,
    minify: minimize,
  };
}

function compatStats(
  webpackStats: webpack.Configuration["stats"],
): ConfigComplete["sourceMaps"] {
  return !!webpackStats;
}
