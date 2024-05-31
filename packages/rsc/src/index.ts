import assert from 'assert';

interface MakoStats {
  chunks: {
    type: 'chunk';
    id: string;
    files: string[];
    entry: boolean;
    modules: { type: 'module'; size: number; id: string; chunks: string[] }[];
    siblings: string[];
    origins: unknown[];
  }[];
  modules: Record<
    string,
    {
      id: string;
      dependencies: string[];
      dependents: string[];
    }
  >;
  rscClientComponents: { path: string; moduleId: string }[];
  rscCSSModules: { path: string; moduleId: string; modules: boolean }[];
}

interface ServerManifest {
  rscClientComponents: { path: string; entries: string[] }[];
  rscCSSModules: { path: string; entries: string[] }[];
}

export function parseServerStats(stats: MakoStats): ServerManifest {
  assert(
    stats.modules,
    'modules must be provided in stats, please configured stats.modules to true in your config.',
  );
  let rscClientComponents = stats.rscClientComponents.map((module) => {
    return {
      path: module.path,
      moduleId: module.moduleId,
      entries: findEntries(module.moduleId, stats),
    };
  });
  let rscCSSModules = stats.rscCSSModules.map((module) => {
    return {
      path: module.path,
      moduleId: module.moduleId,
      entries: findEntries(module.moduleId, stats),
    };
  });
  return {
    rscCSSModules,
    rscClientComponents,
  };
}

function findEntries(moduleId: string, stats: MakoStats): string[] {
  let entries: string[] = [];
  let modules = stats.modules;
  let module = modules[moduleId];
  assert(module, `module ${moduleId} not found in stats.modules`);
  let queue = [module];
  while (queue.length) {
    let module = queue.shift();
    if (!module) continue;
    let dependents = module.dependents;
    if (!dependents.length) {
      entries.push(module.id);
      continue;
    }
    for (let dependent of dependents) {
      queue.push(modules[dependent]);
    }
  }
  return entries;
}

interface ClientManifest {
  clientComponents: Record<
    string,
    Record<string, { id: string; name: string; chunks: string[] }>
  >;
}

export function parseClientStats(stats: MakoStats): ClientManifest {
  let ret: ClientManifest = {
    clientComponents: {},
  };
  for (let chunk of stats.chunks) {
    if (chunk.entry) continue;
    let id = chunk.id;
    const chunks = chunk.siblings.concat(chunk.id);
    // TODO: support module_id_strategy: hashed
    ret.clientComponents[id] = {
      '*': {
        id,
        name: '*',
        chunks,
      },
    };
  }
  return ret;
}
