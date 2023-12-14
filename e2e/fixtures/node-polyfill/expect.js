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
assert.match(
  content,
  moduleReg(
    "src/index.tsx",
    'var _path = _interop_require_default._\\(__mako_require__\\("[\\s\\S]*node_modules/node-libs-browser-okam/polyfill/path.js"\\)\\);'
  ),
  "should have polyfill module: path"
);
