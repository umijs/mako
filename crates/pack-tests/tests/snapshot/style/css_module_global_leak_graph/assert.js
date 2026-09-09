const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const marker = "PAGE_A_GLOBAL_ONLY_3275";
const outputDir = path.join(__dirname, "output");
const markerAsset = fs
  .readdirSync(outputDir)
  .filter((asset) => asset.endsWith(".css"))
  .find((asset) =>
    fs.readFileSync(path.join(outputDir, asset), "utf8").includes(marker),
  );

assert(markerAsset, "missing page A global CSS marker in emitted CSS");

const entries = fs
  .readdirSync(outputDir)
  .filter((file) => file.startsWith("turbopack-") && file.endsWith(".js"))
  .map((file) => fs.readFileSync(path.join(outputDir, file), "utf8"));
const pageA = entries.find((content) => content.includes("/input/a.js"));
const pageB = entries.find((content) => content.includes("/input/b.js"));
assert(pageA, "missing page A entry runtime");
assert(pageB, "missing page B entry runtime");

assert(pageA.includes(markerAsset), "page A must load its global CSS asset");
assert(
  !pageB.includes(markerAsset),
  "page B must not load page A global CSS asset",
);
