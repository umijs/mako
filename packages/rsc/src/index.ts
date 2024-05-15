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
  rscClientComponents: { path: string }[];
  rscCSSModules: { path: string }[];
}

interface ServerManifest {
  rscClientComponents: string[];
  rscCSSModules: string[];
}

export function parseServerStats(stats: MakoStats): ServerManifest {
  let rscClientComponents = stats.rscClientComponents.map(
    (module) => module.path,
  );
  let rscCSSModules = stats.rscCSSModules.map((module) => module.path);
  return {
    rscCSSModules,
    rscClientComponents,
  };
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
