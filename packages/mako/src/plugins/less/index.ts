import url from 'url';
import { BuildParams } from '../../';
import { JsHooks } from '../../../binding';
import { RunLoadersOptions } from '../../runLoaders';
import { createParallelLoader } from './parallelLessLoader';

export interface LessLoaderOpts {
  modifyVars?: Record<string, string>;
  globalVars?: Record<string, string>;
  math?:
    | 'always'
    | 'strict'
    | 'parens-division'
    | 'parens'
    | 'strict-legacy'
    | number;
  sourceMap?: any;
  /**
   * A plugin can be a file path string, or a file path string with a params object.
   * Notice! The file path should be a resolved path like require.resolve("less-plugin-clean-css"),
   * and the params object must be a plain json. We will require the plugin file to get the plugin content.
   * If the params object been accepted, that means, the required content will be treated as a factory class of Less.Plugin,
   * we will create a plugin instance with the params object, or else, the required content will be treated as a plugin instance.
   * We do this because the less loader runs in a worker pool for speed, and a less plugin instance can't be passed to worker directly.
   */
  plugins?: (string | [string, Record<string, any>])[];
}

type LessMoulde = {
  id: string;
  deps: Set<LessMoulde>;
  missingDeps: Set<LessMoulde>;
  parents: Set<LessMoulde>;
};

export class LessPlugin implements JsHooks {
  name: string;
  parallelLessLoader: ReturnType<typeof createParallelLoader> | undefined;
  params: BuildParams & { resolveAlias: Record<string, string> };
  extOpts: RunLoadersOptions;
  lessOptions: LessLoaderOpts;
  moduleGraph: Map<string, LessMoulde> = new Map(); // 暂未使用

  constructor(params: BuildParams & { resolveAlias: Record<string, string> }) {
    this.name = 'less-plugin';
    this.params = params;
    this.extOpts = {
      alias: params.resolveAlias,
      root: params.root,
    };
    this.lessOptions = {
      modifyVars: params.config.less?.modifyVars || {},
      globalVars: params.config.less?.globalVars,
      math: params.config.less?.math,
      sourceMap: params.config.less?.sourceMap || false,
      plugins: params.config.less?.plugins || [],
    };
  }

  load: (
    filePath: string,
  ) => Promise<{ content: string; type: 'css' } | undefined> = async (
    filePath: string,
  ) => {
    if (!isTargetFile(filePath)) {
      return;
    }

    const filename = getFilename(filePath);

    let module = this.moduleGraph.get(filename);
    if (!module) {
      module = {
        id: filename,
        deps: new Set(),
        missingDeps: new Set(),
        parents: new Set(),
      };

      this.moduleGraph.set(filename, module);
    }

    this.parallelLessLoader ||= createParallelLoader();
    const result = await this.parallelLessLoader.run({
      filename,
      opts: this.lessOptions,
      extOpts: this.extOpts,
    });

    let content: string = '';

    if (result.result) {
      const buf = result.result[0];
      if (Buffer.isBuffer(buf)) {
        content = buf.toString('utf-8');
      } else {
        content = buf ?? '';
      }
    }

    if (result.fileDependencies?.length) {
      const deps = new Set(
        result.fileDependencies.filter((dep) => dep !== filename),
      );

      for (let dep of deps) {
        let childModule = this.moduleGraph.get(dep);
        if (!childModule) {
          childModule = {
            id: dep,
            deps: new Set(),
            missingDeps: new Set(),
            parents: new Set(),
          };

          this.moduleGraph.set(dep, childModule);
        }

        childModule.parents.add(module);
        module.deps.add(childModule);
      }
    }

    if (result.missingDependencies?.length) {
      const missingDeps = new Set(result.missingDependencies);
      for (let dep of missingDeps) {
        let childModule = this.moduleGraph.get(dep);
        if (!childModule) {
          childModule = {
            id: dep,
            deps: new Set(),
            missingDeps: new Set(),
            parents: new Set(),
          };

          this.moduleGraph.set(dep, childModule);
        }

        childModule.parents.add(module);
        module.missingDeps.add(childModule);
      }
    }

    return {
      content,
      type: 'css',
    };
  };

  generateEnd = () => {
    if (!this.params.watch) {
      this.parallelLessLoader?.destroy();
      this.parallelLessLoader = undefined;
    }
  };
}

function getFilename(filePath: string) {
  let filename = '';
  try {
    filename = decodeURIComponent(url.parse(filePath).pathname || '');
  } catch (e) {
    return '';
  }

  return filename;
}

function isTargetFile(filePath: string) {
  let filename = getFilename(filePath);

  if (filename?.endsWith('.less')) {
    return true;
  }

  return false;
}
