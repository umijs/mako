const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("fs", "module.exports = '';"),
  "should have empty module: fs"
);
assert.match(
  content,
  moduleReg("fs/promise", "module.exports = '';"),
  "should have empty module: fs/promise"
);
assert(
  content.includes(`var _path = /*#__PURE__*/ _interop_require_default._(__mako_require__("../../../node_modules/.pnpm/node-libs-browser-okam`),
  "should have polyfill module: path"
);
assert(
  content.includes(`exports.setTimeout = function() {`),
  "should have polyfill module: timers"
);
