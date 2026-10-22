const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const multimediaPlugin = { VideoComponent: class VideoComponent {} };
const deepPlugin = { deep: true };
const legacySimplePlugin = { legacySimple: true };
const legacyHyphenPlugin = { legacyHyphen: true };
const literalPlugin = { literal: true };

globalThis.window = globalThis;
globalThis.ge = {
  multimediaPlugin,
  plugins: { media: { multimediaPlugin: deepPlugin } },
};
globalThis.legacySimplePlugin = legacySimplePlugin;
globalThis["legacy-plugin"] = legacyHyphenPlugin;
globalThis["ge.multimediaPlugin"] = literalPlugin;
delete globalThis.missingLegacyPlugin;
globalThis.TURBOPACK = [];

const outputDir = path.join(__dirname, "output");
const chunkFiles = fs.readdirSync(outputDir).filter((file) => {
  if (!file.endsWith(".js") || file.startsWith("turbopack-")) {
    return false;
  }

  return fs.readFileSync(path.join(outputDir, file), "utf8").includes(", global)");
});
assert.ok(chunkFiles.length > 0, "expected a chunk containing global externals");

for (const file of chunkFiles) {
  require(path.join(outputDir, file));
}

const factories = new Map();
for (const registration of globalThis.TURBOPACK) {
  for (let index = 1; index < registration.length; index += 2) {
    const id = registration[index];
    if (
      typeof id === "string" &&
      id.includes("[external]") &&
      id.endsWith(", global)")
    ) {
      factories.set(id, registration[index + 1]);
    }
  }
}

function evaluateExternal(request) {
  const entry = [...factories].find(([id]) =>
    id.endsWith(`[external] (${request}, global)`),
  );
  assert.ok(entry, `expected a global external factory for ${request}`);

  let exported;
  let exportCount = 0;
  const result = entry[1]({
    a() {
      throw new Error("global externals must stay synchronous");
    },
    v(value) {
      exported = value;
      exportCount += 1;
    },
  });
  assert.strictEqual(result, undefined);
  assert.strictEqual(exportCount, 1, "the factory must export synchronously");
  return exported;
}

assert.strictEqual(
  evaluateExternal("window ge multimediaPlugin").VideoComponent,
  multimediaPlugin.VideoComponent,
);
assert.strictEqual(evaluateExternal("ge multimediaPlugin"), multimediaPlugin);
assert.strictEqual(evaluateExternal("ge plugins media multimediaPlugin"), deepPlugin);
assert.strictEqual(evaluateExternal("legacySimplePlugin"), legacySimplePlugin);
assert.strictEqual(evaluateExternal("legacy-plugin"), legacyHyphenPlugin);
assert.strictEqual(evaluateExternal("missingLegacyPlugin"), undefined);
assert.strictEqual(evaluateExternal("ge.multimediaPlugin"), literalPlugin);
