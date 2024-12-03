import fs from 'fs';
import path from 'path';
import { FederationConfig } from '.';
import { JsHooks } from '../../binding';

interface Assets {
  js: { async: string[]; sync: string[] };
  css: { async: string[]; sync: string[] };
}

interface SharedConfig {
  id: string;
  name: string;
  version: string;
  singleton: boolean;
  requiredVersion: string;
  assets: Assets;
}

interface RemoteConfig {
  federationContainerName: string;
  moduleName: string;
  alias: string;
  entry: string;
}

interface ExposeConfig {
  id: string;
  name: string;
  assets: Assets;
  path: string;
}

interface TypesConfig {
  path: string;
  name: string;
  zip: string;
  api: string;
}

interface Manifest {
  id: string;
  name: string;
  metaData: {
    name: string;
    type: string;
    buildInfo: {
      buildVersion: string;
      buildName: string;
    };
    remoteEntry: {
      name: string;
      path: string;
      type: string;
    };
    types: TypesConfig;
    globalName: string;
    pluginVersion: string;
    publicPath: string;
  };
  shared: SharedConfig[];
  remotes: RemoteConfig[];
  exposes: ExposeConfig[];
}

export function generateMFManifest(
  root: string,
  federationConfig: FederationConfig,
  bundlerStats: Parameters<Required<JsHooks>['generateEnd']>[0],
) {
  let pkgJson = JSON.parse(
    fs.readFileSync(path.join(root, 'package.json'), 'utf8'),
  );
  let manifest: Manifest = {
    id: pkgJson.name || '',
    name: pkgJson.name || '',
    metaData: {
      name: pkgJson.name || '',
      type: 'app',
      buildInfo: {
        buildVersion: pkgJson.version || '',
        buildName: pkgJson.name || '',
      },
      remoteEntry: {
        name:
          typeof federationConfig.exposes === 'object' &&
          Object.keys(federationConfig.exposes).length > 0
            ? `${federationConfig.name}.js`
            : '',
        path: '',
        type: 'global',
      },
      types: {
        path: '',
        name: '',
        zip: '@mf-types.zip',
        api: '@mf-types.d.ts',
      },
      globalName: federationConfig.name,
      pluginVersion: '0.0.0',
      publicPath: 'auto',
    },
    exposes: federationConfig.exposes
      ? Object.entries(federationConfig.exposes).map(([name, path]) => ({
          id: `${federationConfig.name}:${name.replace(/^\.\//, '')}`,
          name: name.replace(/^\.\//, ''),
          path,
          assets: {
            js: {
              sync: ['__mf_expose_App-async.js'],
              async: [],
            },
            css: {
              sync: [],
              async: [],
            },
          },
        }))
      : [],
    shared: [],
    remotes: bundlerStats.stats.chunkModules
      .filter((m) => m.id.startsWith('mako/container/remote'))
      .map((m) => {
        const data = m.id.split('/');
        return {
          moduleName: data[4],
          federationContainerName: data[3],
          alias: data[3],
          entry: federationConfig.remotes![data[3]].split('@')[1],
        };
      }),
  };
  fs.writeFileSync(
    path.join(root, './dist/mf-manifest.json'),
    JSON.stringify(manifest, null, 2),
  );

  fs.writeFileSync(
    path.join(root, './dist/stats.json'),
    JSON.stringify(bundlerStats.stats, null, 2),
  );
}
