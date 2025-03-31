import path from 'path';
import url from 'url';
import { type Options } from 'sass';
import { BuildParams } from '../../';
import * as binding from '../../../binding';
import { RunLoadersOptions, createParallelLoader } from '../../runLoaders';

type SassModule = {
  id: string;
  deps: Set<SassModule>;
  missing_deps: Set<SassModule>;
  ancestors: Set<SassModule>;
};

export class SassPlugin implements binding.JsHooks {
  name: string;
  params: BuildParams & { resolveAlias: Record<string, string> };
  sassOptions: Options<'async'>;
  extOpts: RunLoadersOptions;
  moduleGraph: Map<string, SassModule> = new Map();
  parallelLoader: ReturnType<typeof createParallelLoader> | undefined;

  constructor(params: BuildParams & { resolveAlias: Record<string, string> }) {
    this.name = 'sass';
    this.params = params;
    this.extOpts = {
      alias: params.resolveAlias,
      root: params.root,
    };
    this.sassOptions = params.config?.sass || {};
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
        missing_deps: new Set(),
        ancestors: new Set(),
      };
      this.moduleGraph.set(filename, module);
    }

    this.parallelLoader ||= createParallelLoader(
      path.resolve(__dirname, './render.js'),
    );
    const result = await this.parallelLoader.run({
      filename,
      opts: this.sassOptions,
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
      for (const dep of deps) {
        let depModule = this.moduleGraph.get(dep);
        if (!depModule) {
          depModule = {
            id: dep,
            deps: new Set(),
            missing_deps: new Set(),
            ancestors: new Set(),
          };
          this.moduleGraph.set(dep, depModule);
        }
        module.deps.add(depModule);
        depModule.ancestors.add(module);
      }
    }
    if (result.missingDependencies?.length) {
      const missingDeps = new Set(result.missingDependencies);
      for (const dep of missingDeps) {
        let depModule = this.moduleGraph.get(dep);
        if (!depModule) {
          depModule = {
            id: dep,
            deps: new Set(),
            missing_deps: new Set(),
            ancestors: new Set(),
          };
          this.moduleGraph.set(dep, depModule);
        }
        module.missing_deps.add(depModule);
        depModule.ancestors.add(module);
      }
    }

    return {
      content,
      type: 'css',
    };
  };

  beforeRebuild = async (paths: string[]) => {
    const result = new Set<string>();

    paths.forEach((filePath) => {
      if (!isTargetFile(filePath)) {
        result.add(filePath);
        return;
      }

      const filename = getFilename(filePath);
      const module = this.moduleGraph.get(filename);

      if (!module || module.ancestors.size === 0) {
        result.add(filePath);
        return;
      }

      module.ancestors.forEach((ancestor) => {
        result.add(ancestor.id);
      });
    });

    return Array.from(result);
  };

  generateEnd = () => {
    if (!this.params.watch) {
      this.parallelLoader?.destroy();
      this.parallelLoader = undefined;
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

  if (filename?.endsWith('.scss')) {
    return true;
  }

  return false;
}
