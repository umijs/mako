import fs from 'fs';
import path from 'path';
import { omit } from 'lodash';
import * as binding from '../binding';
import { ForkTSChecker as ForkTSChecker } from './forkTSChecker';
import { LessLoaderOpts, lessLoader } from './lessLoader';

interface ExtraBuildParams {
  less?: LessLoaderOpts;
  forkTSChecker?: boolean;
}

type BuildParams = binding.BuildParams & ExtraBuildParams;
export { BuildParams };

// ref:
// https://github.com/vercel/next.js/pull/51883
function blockStdout() {
  if (process.platform === 'darwin') {
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
}

export async function build(params: BuildParams) {
  blockStdout();

  params.hooks = params.hooks || {};
  params.config.resolve = params.config.resolve || {};
  params.config.resolve.alias = params.config.resolve.alias || {};

  let makoConfig: any = {};
  const makoConfigPath = path.join(params.root, 'mako.config.json');
  if (fs.existsSync(makoConfigPath)) {
    try {
      makoConfig = JSON.parse(fs.readFileSync(makoConfigPath, 'utf-8'));
    } catch (e: any) {
      throw new Error(`Parse mako.config.json failed: ${e.message}`);
    }
  }

  // alias for: helpers, node-libs, react-refresh, react-error-overlay
  params.config.resolve.alias = {
    ...makoConfig.resolve?.alias,
    ...params.config.resolve.alias,
    // we still need @swc/helpers
    // since features like decorator or legacy browser support will
    // inject helper functions in the build transform step
    '@swc/helpers': path.dirname(require.resolve('@swc/helpers/package.json')),
    'node-libs-browser-okam': path.dirname(
      require.resolve('node-libs-browser-okam/package.json'),
    ),
    'react-refresh': path.dirname(
      require.resolve('react-refresh/package.json'),
    ),
    'react-error-overlay': path.dirname(
      require.resolve('react-error-overlay/package.json'),
    ),
  };

  // built-in less-loader
  let less = lessLoader(null, {
    modifyVars: params.less?.modifyVars || {},
    math: params.less?.math,
    sourceMap: params.less?.sourceMap || false,
    plugins: [
      ['less-plugin-resolve', { aliases: params.config.resolve.alias! }],
      ...(params.less?.plugins || []),
    ],
  });
  let originLoad = params.hooks.load;
  // TODO: improve load binding, should support return null if not matched
  // @ts-ignore
  params.hooks.load = async function (filePath: string) {
    // user load first
    if (originLoad) {
      let originResult = await originLoad(filePath);
      if (originResult) {
        return originResult;
      }
    }
    let lessResult = await less(filePath);
    if (lessResult) {
      return lessResult;
    }
  };

  // in watch mode, we can reuse the worker pool, no need to terminate
  if (!params.watch) {
    params.hooks.generateEnd = () => {
      lessLoader.terminate();
    };
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

  const buildParams = omit(params, ['less', 'forkTSChecker']);

  await binding.build(buildParams);

  if (params.forkTSChecker) {
    const forkTypeChecker = new ForkTSChecker({
      root: params.root,
      watch: params.watch,
    });
    forkTypeChecker.runTypeCheckInChildProcess();
  }
}
