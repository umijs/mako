import fs from "fs";
import path from "path";
import type { NapiWrittenEndpoint } from "../binding";

export interface Assets {
  js: string[];
  css: string[];
}

function isJavascriptAsset(file: string): boolean {
  return (
    file.endsWith(".js") &&
    !file.endsWith(".LICENSE.txt") &&
    !file.endsWith(".map")
  );
}

export function getInitialAssetsFromStats(outputDir: string): Assets {
  const assets = { js: new Set<string>(), css: new Set<string>() };
  const statsPath = path.join(outputDir, "stats.json");
  if (fs.existsSync(statsPath)) {
    try {
      const stats = JSON.parse(fs.readFileSync(statsPath, "utf-8"));
      if (stats.entrypoints) {
        Object.values(stats.entrypoints).forEach((entrypoint: any) => {
          entrypoint.assets?.forEach((asset: any) => {
            if (asset.name.endsWith(".js")) {
              assets.js.add(asset.name);
            }
            if (asset.name.endsWith(".css")) {
              assets.css.add(asset.name);
            }
          });
        });
      }
    } catch (e) {
      console.warn("Failed to read stats.json for assets discovery", e);
    }
  }
  return { js: [...assets.js], css: [...assets.css] };
}

export function getInitialAssetsFromEndpointPaths(
  endpoints: NapiWrittenEndpoint[],
): Assets {
  const assets = { js: new Set<string>(), css: new Set<string>() };

  endpoints.forEach((endpoint) => {
    endpoint.clientPaths.forEach((file) => {
      if (isJavascriptAsset(file)) {
        assets.js.add(file);
      }
      if (file.endsWith(".css")) {
        assets.css.add(file);
      }
    });
  });

  return { js: [...assets.js], css: [...assets.css] };
}
