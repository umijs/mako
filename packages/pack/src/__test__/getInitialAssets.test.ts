import fs from "fs";
import os from "os";
import path from "path";
import { afterEach, describe, expect, it } from "vitest";
import {
  getInitialAssetsFromEndpointPaths,
  getInitialAssetsFromStats,
} from "../utils/getInitialAssets";

let tempDirs: string[] = [];

function createTempDir() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "utoo-assets-"));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs) {
    fs.rmSync(dir, { force: true, recursive: true });
  }
  tempDirs = [];
});

describe("initial asset discovery", () => {
  it("preserves first-seen asset order across overlapping webpack entries", () => {
    const outputDir = createTempDir();
    fs.writeFileSync(
      path.join(outputDir, "stats.json"),
      JSON.stringify({
        entrypoints: {
          main: {
            assets: [
              { name: "runtime.js" },
              { name: "shared.css" },
              { name: "index.js" },
              { name: "index.js" },
              { name: "index.css" },
            ],
          },
          admin: {
            assets: [
              { name: "runtime.js" },
              { name: "admin.js" },
              { name: "shared.css" },
              { name: "admin.css" },
              { name: "index.js" },
              { name: "index.css" },
              { name: "admin.js.map" },
              { name: "logo.svg" },
            ],
          },
        },
      }),
    );

    expect(getInitialAssetsFromStats(outputDir)).toEqual({
      js: ["runtime.js", "index.js", "admin.js"],
      css: ["shared.css", "index.css", "admin.css"],
    });
  });

  it("preserves first-seen client asset order across overlapping endpoints", () => {
    expect(
      getInitialAssetsFromEndpointPaths([
        {
          type: "nodejs",
          entryPath: "dist",
          clientPaths: [
            "static/runtime.js",
            "static/runtime.js",
            "static/runtime.js.map",
            "static/runtime.js.LICENSE.txt",
            "static/shared.css",
            "static/app.js",
            "static/app.css",
          ],
          serverPaths: [{ path: "server/app.js", contentHash: "app" }],
          config: {},
        },
        {
          type: "nodejs",
          entryPath: "dist",
          clientPaths: [
            "static/runtime.js",
            "static/admin.js",
            "static/shared.css",
            "static/admin.css",
            "static/app.js",
            "static/app.css",
            "static/admin.css.map",
            "static/logo.svg",
          ],
          serverPaths: [{ path: "server/admin.js", contentHash: "admin" }],
          config: {},
        },
      ]),
    ).toEqual({
      js: ["static/runtime.js", "static/app.js", "static/admin.js"],
      css: ["static/shared.css", "static/app.css", "static/admin.css"],
    });
  });
});
