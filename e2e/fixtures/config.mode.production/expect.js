const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  /console.log\("production"\)/,
  "should replace process.env.NODE_ENV to production"
);
assert.doesNotMatch(content, /  /, "should not have space");
assert.doesNotMatch(
  content,
  /function\(module, exports, require\) \{/,
  "should minimize"
);
