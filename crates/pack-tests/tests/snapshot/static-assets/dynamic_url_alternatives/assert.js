const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const issuesDir = path.join(process.cwd(), "issues");
if (fs.existsSync(issuesDir)) {
  for (const name of fs.readdirSync(issuesDir)) {
    assert.doesNotMatch(
      fs.readFileSync(path.join(issuesDir, name), "utf8"),
      /Module not found/,
      "dynamic alternatives must not produce a resolution error",
    );
  }
}

const outputDir = path.join(process.cwd(), "output");
const sources = fs
  .readdirSync(outputDir)
  .filter((name) => name.endsWith(".js") && !name.startsWith("turbopack-"))
  .map((name) => fs.readFileSync(path.join(outputDir, name), "utf8"));
const context = vm.createContext({ URL, TURBOPACK: [] });
for (const source of sources) {
  if (source.includes('globalThis["TURBOPACK"]')) {
    vm.runInContext(source, context);
  }
}

const factories = new Map();
for (const registration of context.TURBOPACK) {
  for (let i = 1; i < registration.length; i += 2) {
    factories.set(registration[i], registration[i + 1]);
  }
}
const entry = [...factories].find(([id]) => id.includes("input/index.js"));
assert.ok(entry, "expected the entry module factory");
const workerEntry = [...factories].find(([id]) =>
  id.endsWith("input/worker.js [client] (ecmascript)"),
);
assert.ok(workerEntry, "the static Worker fallback must still be bundled");
const workerMessages = [];
context.self = { postMessage: (message) => workerMessages.push(message) };
workerEntry[1]({}, {}, {});
assert.deepEqual(workerMessages, ["static worker ready"]);

for (const contents of ["static fallback asset", "bounded asset"]) {
  assert.ok(
    fs.readdirSync(outputDir).some(
      (name) =>
        name.endsWith(".txt") &&
        fs.readFileSync(path.join(outputDir, name), "utf8").trim() === contents,
    ),
    `expected the emitted asset: ${contents.trim()}`,
  );
}

const base = "https://example.test/modules/index.js";
const bundledRequests = [];
entry[1]({
  F: () => base,
  r(id) {
    bundledRequests.push(id);
    assert.ok(factories.has(id), `missing bundled reference: ${id}`);
    return (Worker, options) => new Worker("bundled-static-worker", options);
  },
});

context.Worker = class {
  constructor(url, options) {
    this.url = String(url);
    this.options = options;
  }
};

for (const url of ["./custom.js", "https://example.test/custom.js", "blob:test"]) {
  const worker = context.createWorker({ classWorkerURL: url, type: "module" });
  assert.equal(worker.url, new URL(url, base).href);
  assert.equal(worker.options.type, "module");
  assert.equal(context.optionalURL(url).href, new URL(url, base).href);
  assert.equal(context.fallbackURL(url).href, new URL(url, base).href);
  assert.equal(context.dynamicURL(url).href, new URL(url, base).href);
}
assert.equal(context.optionalURL().href, new URL(undefined, base).href);
assert.equal(context.fallbackURL().href, new URL("./fallback.txt", base).href);
assert.equal(bundledRequests.length, 0, "dynamic URLs must remain runtime values");

assert.equal(context.createWorker().url, "bundled-static-worker");
assert.equal(bundledRequests.length, 1, "the static fallback must use its loader");
