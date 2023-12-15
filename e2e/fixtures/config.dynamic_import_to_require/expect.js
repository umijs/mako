const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  !(`src_foo_js-async.js` in files),
  "should not have file: src_foo_js-async.js"
);
assert(
  !(`src_foo_js-async.js.map` in files),
  "should not have file: src_foo_js-async.js.map"
);
assert(
  !(`node_modules_foo_index_js-async.js` in files),
  "should not have file: node_modules_foo_index_js-async.js"
);
assert(
  !(`node_modules_foo_index_js-async.js.map` in files),
  "should not have file: node_modules_foo_index_js-async.js.map"
);
