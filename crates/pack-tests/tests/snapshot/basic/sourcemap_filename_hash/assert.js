const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const outputDir = path.join(__dirname, "output");

function listFiles(directory, prefix = "") {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.posix.join(prefix, entry.name);
    return entry.isDirectory()
      ? listFiles(path.join(directory, entry.name), file)
      : [file];
  });
}

function collectSources(map) {
  return [
    ...(map.sources ?? []),
    ...(map.sections ?? []).flatMap((section) => collectSources(section.map)),
  ];
}

const files = listFiles(outputDir).sort();
const assets = files.filter((file) => /\.(?:js|css)$/.test(file));
const scripts = assets.filter((file) => file.endsWith(".js"));
const styles = assets.filter((file) => file.endsWith(".css"));

assert.ok(
  scripts.some((file) => /^scripts\/main\.[a-f0-9]{8}\.js$/.test(file)),
  "the entry must retain its content hash",
);
assert.ok(
  scripts.length >= 4,
  "the entry and both async chunks must be emitted",
);
assert.ok(styles.length >= 3, "initial and both async styles must be emitted");

for (const file of assets) {
  assert.match(file, /\.[a-f0-9]{8}\.(?:async\.js|js|css)$/);
  const content = fs.readFileSync(path.join(outputDir, file), "utf8");
  const comments = [...content.matchAll(/[#@]\s*sourceMappingURL=([^\s*]+)/g)];
  assert.equal(comments.length, 1, `${file} must have one sourceMappingURL`);
  assert.equal(comments[0][1], `${path.posix.basename(file)}.map`);
  const mapFile = `${file}.map`;
  assert.ok(files.includes(mapFile), `missing corresponding map for ${file}`);
  const map = JSON.parse(
    fs.readFileSync(path.join(outputDir, mapFile), "utf8"),
  );
  assert.equal(map.version, 3);
}

assert.deepEqual(
  files.filter((file) => file.endsWith(".map")),
  assets.map((file) => `${file}.map`).sort(),
  "every emitted map must correspond to one JS or CSS asset",
);

for (const module of ["lazy-one", "lazy-two"]) {
  for (const extension of ["js", "css"]) {
    const matchingMaps = assets
      .filter((file) => file.endsWith(`.${extension}`))
      .filter((file) => {
        const map = JSON.parse(
          fs.readFileSync(path.join(outputDir, `${file}.map`), "utf8"),
        );
        return collectSources(map).some((source) =>
          source.endsWith(`/input/${module}.${extension}`),
        );
      });
    assert.equal(
      matchingMaps.length,
      1,
      `${module}.${extension} must have its own corresponding map`,
    );
  }
}
