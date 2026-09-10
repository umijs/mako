const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const acorn = require("acorn");
const { AnyMap, originalPositionFor } = require("@jridgewell/trace-mapping");

const { config } = JSON.parse(fs.readFileSync("config.json", "utf8"));
const legacy = config.target === "Android 4, iOS 8";
const code = fs.readFileSync("output/main.js", "utf8");
acorn.parse(code, { ecmaVersion: legacy ? 5 : 2022, sourceType: "script" });
const sourceMap =
  config.sourceMaps === false
    ? null
    : AnyMap(JSON.parse(fs.readFileSync("output/main.js.map", "utf8")));
if (!legacy) assert.match(code, /=>/);
if (config.optimization.extractComments) {
  assert.match(code, /For license information please see main.js.LICENSE.txt/);
  assert.match(
    fs.readFileSync("output/main.js.LICENSE.txt", "utf8"),
    /@license Library runtime target fixture/,
  );
}

// Dynamic imports must remain in the library, even when its target is a browser.
assert.deepEqual(
  fs.readdirSync("output", { recursive: true }).filter((file) => file.endsWith(".js")),
  ["main.js"],
);

async function checkExports(commonjs, currentScript) {
  const context = {
    console,
    URL,
    document: {
      currentScript,
      getElementsByTagName: () => [{ src: "https://example.test/widgets/main.js" }],
    },
    ...(commonjs ? { module: { exports: {} }, exports: {} } : {}),
    // Older browsers do not provide globalThis. Do not inherit it from Node's VM.
    ...(legacy ? { globalThis: undefined } : {}),
  };
  context.self = context;
  const initialGlobals = Object.keys(context);
  vm.runInNewContext(code, context, { filename: "main.js" });
  assert.deepEqual(
    Object.keys(context).filter((key) => !initialGlobals.includes(key)),
    commonjs ? [] : ["LegacyLibrary"],
  );
  const library = commonjs ? context.module.exports : context.LegacyLibrary;
  assert.equal(library.read(), 42);
  assert.equal(library.read({ answer: 7 }), 7);
  assert.equal(library.flag, 1);
  assert.equal(library.last({ next: { value: 1, next: { value: 2 } } }), 2);
  assert.equal(await library.load(), 43);
  assert.match(library.asset, /^https:\/\/example\.test\/widgets\/.*\.svg$/);
  assert.throws(() => library.fail(), (error) => {
    if (sourceMap) {
      const [, line, column] = error.stack.match(/main\.js:(\d+):(\d+)/);
      const original = originalPositionFor(sourceMap, {
        line: Number(line),
        column: Number(column) - 1,
      });
      assert.match(original.source, /input\/index\.js$/);
      assert.equal(original.line, 11);
    }
    return error.message === "target-map";
  });
}

Promise.all([
  checkExports(false, { src: "https://example.test/widgets/main.js" }),
  checkExports(false, null),
  checkExports(true, { src: "https://example.test/widgets/main.js" }),
]).catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
