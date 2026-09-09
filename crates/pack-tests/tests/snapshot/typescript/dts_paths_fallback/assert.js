const assert = require("node:assert/strict");

const expected = {
  exact: "dts-exact-runtime",
  wildcard: "dts-wild-runtime",
  main: "dts-main-runtime",
  mixed: "first",
  empty: "undefined",
};

assert.deepEqual(require("./output/client/main.js").default, {
  ...expected,
  ignored: "undefined",
});
require("./output/server/index.js");
assert.deepEqual(globalThis.dtsPathsServerResult, expected);
