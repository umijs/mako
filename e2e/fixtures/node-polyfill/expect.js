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
  content.includes(`var _path = /*#__PURE__*/ _interop_require_default._(__mako_require__("../../../node_modules/.pnpm/node-libs-browser-okam@2.2.4/node_modules/node-libs-browser-okam/polyfill/path.js"));`),
  "should have polyfill module: path"
);
