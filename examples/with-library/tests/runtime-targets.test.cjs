const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { test } = require("node:test");
const vm = require("node:vm");
const { AnyMap, originalPositionFor } = require("@jridgewell/trace-mapping");
const { build } = require("@utoo/pack");
const acorn = require("acorn");

const projectPath = path.resolve(__dirname, "..");
const cases = [
  { name: "legacy production", legacy: true, minify: false },
  { name: "legacy minified", legacy: true, minify: true },
  { name: "legacy development", legacy: true, minify: false, dev: true },
  { name: "modern minified", legacy: false, minify: true },
];

for (const { name, legacy, minify, dev = false } of cases) {
  test(name, async () => {
    const outputPath = path.join(projectPath, "dist", "runtime-targets", name);
    try {
      await build(
        {
          dev,
          tracing: false,
          config: {
            entry: [
              {
                import: "./tests/runtime-targets/index.js",
                name: "main",
                library: { name: "LegacyLibrary" },
              },
            ],
            mode: dev ? "development" : "production",
            target: legacy ? "Android 4, iOS 8" : "Chrome 100",
            sourceMaps: !dev,
            optimization: {
              minify,
              moduleIds: "named",
              extractComments: minify,
            },
            output: { path: outputPath, publicPath: "auto", clean: true },
          },
        },
        projectPath,
      );

      const code = fs.readFileSync(path.join(outputPath, "main.js"), "utf8");
      acorn.parse(code, {
        ecmaVersion: legacy ? 5 : 2022,
        sourceType: "script",
      });
      if (!legacy) assert.match(code, /=>/);
      const sourceMap = dev
        ? null
        : AnyMap(
            JSON.parse(
              fs.readFileSync(path.join(outputPath, "main.js.map"), "utf8"),
            ),
          );
      if (minify) {
        assert.match(
          code,
          /For license information please see main.js.LICENSE.txt/,
        );
        assert.match(
          fs.readFileSync(path.join(outputPath, "main.js.LICENSE.txt"), "utf8"),
          /@license Library runtime target fixture/,
        );
      }
      // Dynamic imports must stay in the library, including for browser targets.
      assert.deepEqual(
        fs
          .readdirSync(outputPath, { recursive: true })
          .filter((file) => file.endsWith(".js")),
        ["main.js"],
      );

      for (const [commonjs, currentScript] of [
        [false, { src: "https://example.test/widgets/main.js" }],
        [false, null],
        [true, { src: "https://example.test/widgets/main.js" }],
      ]) {
        const context = {
          console,
          URL,
          document: {
            currentScript,
            getElementsByTagName: () => [
              { src: "https://example.test/widgets/main.js" },
            ],
          },
          ...(commonjs ? { module: { exports: {} }, exports: {} } : {}),
          // Do not inherit Node's globalThis when emulating an older browser.
          ...(legacy ? { globalThis: undefined } : {}),
        };
        context.self = context;
        const initialGlobals = Object.keys(context);
        vm.runInNewContext(code, context, { filename: "main.js" });
        assert.deepEqual(
          Object.keys(context).filter((key) => !initialGlobals.includes(key)),
          commonjs ? [] : ["LegacyLibrary"],
        );
        const library = commonjs
          ? context.module.exports
          : context.LegacyLibrary;
        assert.equal(library.read(), 42);
        assert.equal(library.read({ answer: 7 }), 7);
        assert.equal(library.flag, 1);
        assert.equal(
          library.last({ next: { value: 1, next: { value: 2 } } }),
          2,
        );
        assert.equal(await library.load(), 43);
        assert.match(
          library.asset,
          /^https:\/\/example\.test\/widgets\/.*\.svg$/,
        );
        assert.throws(
          () => library.fail(),
          (error) => {
            if (sourceMap) {
              const [, line, column] =
                error.stack.match(/main\.js:(\d+):(\d+)/);
              const original = originalPositionFor(sourceMap, {
                line: Number(line),
                column: Number(column) - 1,
              });
              assert.match(original.source, /runtime-targets\/index\.js$/);
              assert.equal(original.line, 11);
            }
            return error.message === "target-map";
          },
        );
      }
    } finally {
      fs.rmSync(outputPath, { recursive: true, force: true });
    }
  });
}
