const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  /console.log\("development"\)/,
  "should replace process.env.NODE_ENV to development"
);
assert.match(content, /  /, "should have space");
assert.match(
  content,
  /function\(module, exports, __mako_require__\) \{/,
  "should not minimize"
);
