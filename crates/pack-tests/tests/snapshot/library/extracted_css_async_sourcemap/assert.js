const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const outputDir = path.join(__dirname, "output");
const outputFiles = fs.readdirSync(outputDir);
const css = fs.readFileSync(path.join(outputDir, "main.css"), "utf8");
const sourceMaps = require("./config.json").config.sourceMaps;

assert.deepEqual(outputFiles.filter((file) => file.endsWith(".css")), ["main.css"]);
assert.deepEqual(outputFiles.filter((file) => file.endsWith(".js")), ["library.js"]);
assert.match(css, /\.initial/);
assert.match(css, /\.lazy/);
assert.ok(css.indexOf(".initial") < css.indexOf(".lazy"), "preserve CSS import order");

if (sourceMaps) {
  assert.deepEqual(outputFiles.filter((file) => file.endsWith(".css.map")), ["main.css.map"]);
  assert.match(css, /sourceMappingURL=main\.css\.map/);
  const map = JSON.parse(fs.readFileSync(path.join(outputDir, "main.css.map"), "utf8"));
  const maps = map.sections ? map.sections.map((section) => section.map) : [map];
  const sources = maps.flatMap((section) => section.sources);
  for (const source of ["input/initial.css", "input/lazy.css"]) {
    assert.ok(sources.some((file) => file.endsWith(source)), `missing ${source}`);
  }
} else {
  assert.ok(!outputFiles.some((file) => file.endsWith(".map")));
}
