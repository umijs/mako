const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const outputDir = path.join(__dirname, "output");
const baseURL = "https://example.test/";
globalThis.TURBOPACK = [];
for (const file of fs.readdirSync(outputDir)) {
  if (file.endsWith(".js")) require(path.join(outputDir, file));
}

// Snapshots use a dummy runtime. Execute the emitted factories with URL helpers.
const factories = new Map();
for (const chunk of globalThis.TURBOPACK) {
  for (let index = 1; index < chunk.length; index += 2) {
    if (typeof chunk[index + 1] === "function") {
      factories.set(chunk[index], chunk[index + 1]);
    }
  }
}
function load(id) {
  let exported;
  const factory = factories.get(id);
  assert(factory, `missing module ${id}`);
  factory(
    {
      F: (file) => new URL(file, baseURL).href,
      U: class extends URL {
        constructor(value) {
          super(value, baseURL);
        }
      },
      q: (value) => {
        exported = value;
      },
      r: load,
    },
    {},
    {},
  );
  return exported;
}
load([...factories.keys()].find((id) => id.includes("/input/index.js ")));

assert.equal(
  globalThis.fallbackURL("./custom.txt").href,
  new URL("input/custom.txt", baseURL).href,
);
assert.equal(
  globalThis.fallbackURL("").href,
  new URL("input/index.js", baseURL).href,
);
const fallback = globalThis.fallbackURL();
assert.equal(globalThis.fallbackURL(null).href, fallback.href);
assert.equal(
  fs.readFileSync(
    path.join(outputDir, path.basename(fallback.pathname)),
    "utf8",
  ),
  "static fallback",
);

globalThis.Worker = class {
  constructor(url) {
    this.url = url;
  }
};
const customWorker = "https://example.test/custom-worker.js";
assert.equal(
  globalThis.createWorker({ classWorkerURL: customWorker }).url.href,
  customWorker,
);
assert.equal(globalThis.createWorker(), undefined);
