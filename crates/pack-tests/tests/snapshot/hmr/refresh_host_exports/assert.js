const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

function main() {
  const context = vm.createContext({ console });
  context.self = context;
  const output = path.join(__dirname, "output");
  for (const file of fs
    .readdirSync(output)
    .filter((file) => file.endsWith(".js"))) {
    vm.runInContext(fs.readFileSync(path.join(output, file), "utf8"), context, {
      filename: file,
    });
  }

  // Dummy chunks register factories without executing them. Implement only the
  // module primitives used by this fixture; Refresh and SWC code stay real.
  const factories = new Map();
  const entries = [];
  for (const chunk of context.TURBOPACK) {
    entries.push(...(chunk[1]?.runtimeModuleIds ?? []));
    for (let index = 1; index < chunk.length; index += 2) {
      if (typeof chunk[index + 1] === "function") {
        factories.set(chunk[index], chunk[index + 1]);
      }
    }
  }
  const modules = new Map();
  function load(id) {
    if (modules.has(id)) return modules.get(id);
    const factory = factories.get(id);
    assert(factory, `missing module ${id}`);
    const module = { exports: {} };
    modules.set(id, module);
    factory(
      {
        m: module,
        r: (id) => load(id).exports,
        s(bindings) {
          module.namespace = module.exports;
          for (let index = 0; index < bindings.length; index += 2) {
            Object.defineProperty(module.exports, bindings[index], {
              get: bindings[index + 1],
            });
          }
        },
        i(id) {
          const imported = load(id);
          return (imported.namespace ??= {
            ...imported.exports,
            default: imported.exports,
          });
        },
      },
      module,
      module.exports,
    );
    return module;
  }
  assert.equal(entries.length, 1);
  load(entries[0]);
  const { helpers, runtime, userTypeof } = context.refreshHostExports;

  const reads = [];
  const host = new Proxy(
    {},
    {
      get(target, key) {
        reads.push(key);
        if (key === "constructor" || key === "$$typeof") {
          throw new DOMException(
            "Cross-origin Window property",
            "SecurityError",
          );
        }
        return Reflect.get(target, key);
      },
    },
  );

  // Model the access policy of a cross-origin WindowProxy. The compiled Refresh
  // code, including its real SWC helpers, must never probe its constructor.
  runtime.register(host, "host");
  helpers.registerExportsForReactRefresh({ 0: host }, "window-export");
  helpers.registerExportsForReactRefresh(host, "host-export");
  assert.equal(helpers.isReactRefreshBoundary({ childWindow: host }), false);
  helpers.getRefreshBoundarySignature({ childWindow: host });
  assert.ok(!reads.includes("constructor"));

  const { Component, memo, forwardRef } = vm.runInContext(
    `(() => {
    function Component() {}
    return {
      Component,
      memo: { $$typeof: Symbol.for("react.memo"), type: Component },
      forwardRef: { $$typeof: Symbol.for("react.forward_ref"), render: function Render() {} }
    };
  })()`,
    context,
  );
  helpers.registerExportsForReactRefresh(
    { Component, memo, forwardRef },
    "components",
  );
  assert.equal(
    helpers.isReactRefreshBoundary({ Component, memo, forwardRef }),
    true,
  );
  assert.equal(runtime.getFamilyByType(memo).current, memo);
  assert.equal(runtime.getFamilyByType(Component).current, Component);
  assert.equal(
    runtime.getFamilyByType(forwardRef.render).current,
    forwardRef.render,
  );

  // The exclusion belongs only to Refresh, not the application's target settings.
  assert.throws(() => userTypeof(host), { name: "SecurityError" });
}

main();
