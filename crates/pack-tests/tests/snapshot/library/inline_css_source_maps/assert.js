const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const outputDir = path.join(__dirname, "output");
const outputFiles = fs.readdirSync(outputDir);
const bundle = fs.readFileSync(path.join(outputDir, "library.min.js"), "utf8");
const cssMaps = outputFiles.filter((file) => file.endsWith(".css.map"));
const referencedMaps = [
  ...bundle.matchAll(/sourceMappingURL=([^*]+\.css\.map)\*\//g),
].map((match) => decodeURIComponent(match[1]));

assert.equal(cssMaps.length, 3, "each inline CSS chunk needs its own map");
assert.deepEqual([...new Set(referencedMaps)].sort(), cssMaps.sort());
assert.ok(!outputFiles.some((file) => file.endsWith(".css")));
assert.ok(outputFiles.includes("library.min.js.map"));
assert.match(bundle, /\.token\{/);

const sources = cssMaps.flatMap((file) => {
  const map = JSON.parse(fs.readFileSync(path.join(outputDir, file), "utf8"));
  return map.sources;
});
for (const source of ["input/shared.css", "input/a/style.css", "input/b/style.css"]) {
  assert.ok(sources.some((file) => file.endsWith(source)), `missing ${source}`);
}
