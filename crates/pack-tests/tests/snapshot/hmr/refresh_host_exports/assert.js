const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

async function main() {
  const context = vm.createContext({ console, URL });
  context.self = context;
  // Every emitted chunk is preloaded below; model its existing script element.
  context.document = { querySelectorAll: () => [{ addEventListener() {} }] };
  const output = path.join(__dirname, "output");
  for (const file of fs
    .readdirSync(output)
    .filter((file) => file.endsWith(".js"))) {
    context.document.currentScript = { getAttribute: () => file };
    vm.runInContext(fs.readFileSync(path.join(output, file), "utf8"), context, {
      filename: file,
    });
  }
  await new Promise(setImmediate);
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

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
