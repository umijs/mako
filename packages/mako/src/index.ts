import fs from 'fs';
import os from 'os';
import path from 'path';
import { omit } from 'lodash';
import resolve from 'resolve';
import { type Options } from 'sass';
import * as binding from '../binding';
import { ForkTSChecker as ForkTSChecker } from './forkTSChecker';
import { LessLoaderOpts, lessLoader } from './lessLoader';
import { sassLoader } from './sassLoader';

type Config = binding.BuildParams['config'] & {
  plugins?: binding.BuildParams['plugins'];
  less?: LessLoaderOpts;
  sass?: Options<'async'> & { resources: string[] };
  forkTSChecker?: boolean;
};

type BuildParams = {
  config: Config;
  root: binding.BuildParams['root'];
  watch: binding.BuildParams['watch'];
};

export { BuildParams };

// ref:
// https://github.com/vercel/next.js/pull/51883
function blockStdout() {
  // rust needs stdout to be blocking, otherwise it will throw an error (on macOS at least) when writing a lot of data (logs) to it
  // see https://github.com/napi-rs/napi-rs/issues/1630
  // and https://github.com/nodejs/node/blob/main/doc/api/process.md#a-note-on-process-io
  if ((process.stdout as any)._handle != null) {
    (process.stdout as any)._handle.setBlocking(true);
  }
  if ((process.stderr as any)._handle != null) {
    (process.stderr as any)._handle.setBlocking(true);
  }
}

export async function build(params: BuildParams) {
  blockStdout();

  params.config.plugins = params.config.plugins || [];
  params.config.resolve = params.config.resolve || {};

  let makoConfig: any = {};
  let makoConfigPath = path.join(params.root, 'mako.config.json');
  if (fs.existsSync(makoConfigPath)) {
    try {
      makoConfig = JSON.parse(fs.readFileSync(makoConfigPath, 'utf-8'));
    } catch (e: any) {
      throw new Error(`Parse mako.config.json failed: ${e.message}`);
    }
  }

  // alias for: helpers, node-libs, react-refresh, react-error-overlay
  params.config.resolve.alias = [
    ...(makoConfig.resolve?.alias || []),
    ...(params.config.resolve?.alias || []),
    // we still need @swc/helpers
    // since features like decorator or legacy browser support will
    // inject helper functions in the build transform step
    [
      '@swc/helpers',
      path.dirname(require.resolve('@swc/helpers/package.json')),
    ],
    [
      'node-libs-browser-okam',
      path.dirname(require.resolve('node-libs-browser-okam/package.json')),
    ],
    [
      'react-refresh',
      path.dirname(require.resolve('react-refresh/package.json')),
    ],
    [
      'react-error-overlay',
      path.dirname(require.resolve('react-error-overlay/package.json')),
    ],
  ];

  const lessPluginAlias =
    params.config.resolve?.alias?.reduce(
      (accumulator: Record<string, string>, currentValue) => {
        accumulator[currentValue[0]] = currentValue[1];
        return accumulator;
      },
      {},
    ) || {};

  // built-in less-loader
  let less = lessLoader(null, {
    modifyVars: params.config.less?.modifyVars || {},
    globalVars: params.config.less?.globalVars,
    math: params.config.less?.math,
    sourceMap: params.config.less?.sourceMap || false,
    plugins: [
      ['less-plugin-resolve', { aliases: lessPluginAlias }],
      ...(params.config.less?.plugins || []),
    ],
  });
  params.config.plugins.push({
    name: 'less',
    async load(filePath: string) {
      let lessResult = await less.render(filePath);
      if (lessResult) {
        return lessResult;
      }
    },
    generateEnd() {
      if (!params.watch) {
        less.terminate();
      }
    },
  });

  if (makoConfig?.sass || params.config?.sass) {
    const sassOpts = {
      ...(makoConfig?.sass || {}),
      ...(params.config?.sass || {}),
    };
    let sass = sassLoader(null, sassOpts);
    params.config.plugins.push({
      name: 'sass',
      async load(filePath: string) {
        let sassResult = await sass.render(filePath);
        if (sassResult) {
          return sassResult;
        }
      },
      generateEnd() {
        if (!params.watch) {
          sass.terminate();
        }
      },
    });
  }

  // support dump mako config
  if (process.env.DUMP_MAKO_CONFIG) {
    const configFile = path.join(params.root, 'mako.config.json');
    fs.writeFileSync(configFile, JSON.stringify(params.config, null, 2));
  }

  if (process.env.XCODE_PROFILE) {
    await new Promise<void>((resolve) => {
      const readline = require('readline');
      const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout,
      });
      rl.question(
        `Xcode profile enabled. Current process ${process.title} (${process.pid}) . Press Enter to continue...\n`,
        () => {
          rl.close();
          resolve();
        },
      );
    });
  }

  let plugins = params.config.plugins;
  plugins = plugins.map((plugin: any) => {
    if (typeof plugin === 'string') {
      let fn = require(
        resolve.sync(plugin, {
          basedir: params.root,
        }),
      );
      return fn.default || fn;
    } else {
      return plugin;
    }
  });
  makoConfig.plugins?.forEach((plugin: any) => {
    if (typeof plugin === 'string') {
      let fn = require(
        resolve.sync(plugin, {
          basedir: params.root,
        }),
      );
      plugins.push(fn.default || fn);
    } else {
      throw new Error(
        `Invalid plugin: ${plugin} in mako.config.json, only support string type plugin here.`,
      );
    }
  });

  // add context to each plugin's hook
  plugins.forEach((plugin: any) => {
    Object.keys(plugin).forEach((key) => {
      const oldValue = plugin[key];
      if (typeof oldValue === 'function') {
        plugin[key] = (context: any, ...args: any[]) => {
          let result = oldValue.apply(
            {
              // https://rollupjs.org/plugin-development/#this-parse
              parse(_code: string) {
                throw new Error('parse is not supported');
              },
              // https://rollupjs.org/plugin-development/#this-addwatchfile
              addWatchFile(_file: string) {
                throw new Error('addWatchFile is not supported');
              },
              // https://rollupjs.org/plugin-development/#this-emitfile
              // only support asset type
              emitFile(file: {
                type: 'asset' | 'chunk' | 'prebuilt-chunk';
                name?: string;
                fileName?: string;
                source?: string | Uint8Array;
              }) {
                if (file.type !== 'asset') {
                  throw new Error('emitFile only support asset type');
                }
                if (file.name && !file.fileName) {
                  throw new Error(
                    'name in emitFile is not supported yet, please supply fileName instead',
                  );
                }
                // Since assets_info in mako is a <origin_path, output_path> map,
                // we need to generate a tmp file to store the content, and then emit it
                // TODO: we should use a better way to handle this
                const tmpFile = path.join(
                  os.tmpdir(),
                  Math.random().toString(36).substring(2, 15),
                );
                fs.writeFileSync(tmpFile, file.source!);
                context.emitFile(tmpFile, file.fileName!);
              },
              warn(
                message:
                  | string
                  | { message: string; pluginCode?: string; meta?: string },
              ) {
                if (typeof message === 'object') {
                  const msg = [
                    message.message,
                    message.pluginCode
                      ? `pluginCode: ${message.pluginCode}`
                      : '',
                    message.meta ? `meta: ${message.meta}` : '',
                  ]
                    .filter(Boolean)
                    .join('\n');
                  context.warn(msg);
                } else {
                  context.warn(message);
                }
              },
              error(
                message:
                  | string
                  | { message: string; pluginCode?: string; meta?: string },
              ) {
                if (typeof message === 'object') {
                  const msg = [
                    message.message,
                    message.pluginCode
                      ? `pluginCode: ${message.pluginCode}`
                      : '',
                    message.meta ? `meta: ${message.meta}` : '',
                  ]
                    .filter(Boolean)
                    .join('\n');
                  context.error(msg);
                } else {
                  context.error(message);
                }
              },
            },
            [...args],
          );
          // adapter mako hooks for unplugin
          if (key === 'load' || key === 'transform') {
            // if result is null, return the original code
            if (result === null) {
              result = args[0];
            }
            const isPromise = typeof result === 'object' && result.then;
            if (isPromise) {
              result = result.then((result: any) => adapterResult(result));
            } else {
              result = adapterResult(result);
            }
          }
          if (key === 'resolveId') {
            if (typeof result === 'string') {
              result = {
                id: result,
                external: false,
              };
            }
          }
          return result;
        };
      }
    });
  });

  params.config = omit(params.config, [
    'less',
    'sass',
    'forkTSChecker',
    'plugins',
  ]) as BuildParams['config'];
  await binding.build({
    ...params,
    plugins,
  });

  if (params.config.forkTSChecker) {
    let forkTypeChecker = new ForkTSChecker({
      root: params.root,
      watch: params.watch,
    });
    forkTypeChecker.runTypeCheckInChildProcess();
  }
}

function adapterResult(result: any) {
  if (typeof result === 'string') {
    return {
      content: result,
      type: 'tsx',
    };
  } else if (typeof result === 'object' && result.code) {
    return {
      content: result.code,
      type: 'tsx',
    };
  }
  return result;
}
