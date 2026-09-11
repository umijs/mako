const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const acorn = require("acorn");

const code = fs.readFileSync("output/main.js", "utf8");
acorn.parse(code, { ecmaVersion: 5, sourceType: "script" });
assert.deepEqual(
  fs
    .readdirSync("output", { recursive: true })
    .filter((file) => file.endsWith(".js")),
  ["main.js"],
);

async function main() {
  for (const currentScript of [{ src: "https://example.test/main.js" }, null]) {
    const context = {
      console,
      URL,
      globalThis: undefined,
      document: {
        currentScript,
        getElementsByTagName: () => [{ src: "https://example.test/main.js" }],
      },
    };
    context.self = context;
    const initialGlobals = Object.keys(context);
    vm.runInNewContext(code, context, { filename: "main.js" });
    assert.deepEqual(
      Object.keys(context).filter((key) => !initialGlobals.includes(key)),
      ["LegacyLibrary"],
    );
    const library = context.LegacyLibrary;
    assert.equal(library.read(), 42);
    assert.equal(library.read({ answer: 7 }), 7);
    assert.equal(library.flag, 1);
    assert.equal(library.last({ next: { value: 1, next: { value: 2 } } }), 2);
    assert.equal(await library.load(), 43);
    assert.match(library.asset, /^https:\/\/example\.test\/.*\.svg$/);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
