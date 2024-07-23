import fs from 'fs';
import path from 'path';
import { omit } from 'lodash';
import resolve from 'resolve';
import * as binding from '../binding';
import { ForkTSChecker as ForkTSChecker } from './forkTSChecker';
import { LessLoaderOpts, lessLoader } from './lessLoader';
import { sassLoader } from './sassLoader';

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

  if (makoConfig?.sass) {
    let sass = sassLoader(null, makoConfig?.sass);
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
