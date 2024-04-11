import fs from 'fs';
import path from 'path';
import * as binding from '../binding';
import { LessLoaderOpts, lessLoader, terminatePool } from './lessLoader';

process.title = 'okamjs';

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

interface ExtraBuildParams {
  less?: LessLoaderOpts;
}

export async function build(params: binding.BuildParams & ExtraBuildParams) {
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
    alias: params.config.resolve.alias!,
    modifyVars: params.less?.modifyVars || {},
    math: params.less?.math,
    sourceMap: params.less?.sourceMap || false,
  });
  let originLoad = params.hooks.load;
  // TODO: improve load binding, should support return null if not matched
  // @ts-ignore
  params.hooks.load = async function (filePath: string) {
    let lessResult = await less(filePath);
    if (lessResult) {
      return lessResult;
    }
    if (originLoad) {
      let originResult = await originLoad(filePath);
      if (originResult) {
        return originResult;
      }
    }
  };

  // in watch mode, we can reuse the worker pool, no need to terminate
  if (!params.watch) {
    params.hooks.generateEnd = () => {
      terminatePool();
    };
  }

  // support dump mako config
  if (process.env.DUMP_MAKO_CONFIG) {
    const configFile = path.join(params.root, 'mako.config.json');
    fs.writeFileSync(configFile, JSON.stringify(params.config, null, 2));
  }

  if (process.env.XCODE_PROFILE) {
    console.log(`Xcode profile enabled. Current pid: ${process.pid}`);
    await new Promise((r) => setTimeout(r, 10000));
  }

  await binding.build(params);
}
