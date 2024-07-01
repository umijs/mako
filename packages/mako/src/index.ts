import fs from 'fs';
import path from 'path';
import { omit } from 'lodash';
import resolve from 'resolve';
import * as binding from '../binding';
import { ForkTSChecker as ForkTSChecker } from './forkTSChecker';
import { LessLoaderOpts, lessLoader } from './lessLoader';

type Config = binding.BuildParams['config'] & {
  plugins?: binding.BuildParams['plugins'];
  less?: LessLoaderOpts;
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

export async function buildConfig(params: BuildParams, config: string) {
  const target = `config_${Date.now()}`;
  const targetPath = path.join(params.root, `/${target}.js`);
  await binding.build({
    root: params.root,
    config: {
      copy: [],
      emitAssets: false,
      entry: {
        [target]: config,
      },
      devtool: false,
      cjs: true,
      platform: 'node',
      output: {
        path: './',
        mode: 'bundle',
      },
    },
    plugins: [],
    watch: false,
  });
  const configResult = require(targetPath);
  fs.unlink(targetPath, () => {});
  return configResult.default || configResult;
}
const JSON_CONFIG = 'mako.config.json';
export async function resolveConfig(
  params: BuildParams,
): Promise<BuildParams | null> {
  const configFiles: string[] = [
    JSON_CONFIG,
    'mako.config.ts',
    'mako.config.js',
  ];
  for (let i = 0; i < configFiles.length; i++) {
    const target = path.join(params.root, configFiles[i]);
    if (fs.existsSync(target)) {
      const result = buildConfig(params, configFiles[i]);
      if (JSON_CONFIG !== configFiles[i]) {
        fs.writeFileSync(
          path.join(params.root, JSON_CONFIG),
          JSON.stringify(result),
        );
      }
      return result;
    }
  }
  return null;
}

export async function build(params: BuildParams) {
  blockStdout();

  params.config.plugins = params.config.plugins || [];
  params.config.resolve = params.config.resolve || {};

  const makoConfig: any = (await resolveConfig(params)) || {};

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
  params.config = omit(params.config, [
    'less',
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
