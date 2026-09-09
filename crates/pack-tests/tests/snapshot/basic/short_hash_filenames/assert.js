const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const outputDir = path.join(__dirname, "output");
const files = fs.readdirSync(outputDir);
const chunks = files.filter((file) => /\.(js|css)$/.test(file));

assert(chunks.filter((file) => file.endsWith(".js")).length >= 3);
assert.equal(chunks.filter((file) => file.endsWith(".css")).length, 2);
assert(chunks.some((file) => file.startsWith("turbopack-")));

for (const file of files) {
  assert.match(file, /^(?:turbopack-)?[0-9a-z_-]{13}\.(?:js|css)(?:\.map)?$/);
}

// The dummy runtime used by snapshots does not emit a source map comment.
for (const file of chunks.filter((file) => !file.startsWith("turbopack-"))) {
  const content = fs.readFileSync(path.join(outputDir, file), "utf8");
  const sourceMap = content.match(/sourceMappingURL=([^\s*]+)/);
  assert(sourceMap, `missing source map reference in ${file}`);
  assert(files.includes(sourceMap[1]), `missing source map ${sourceMap[1]}`);
}
