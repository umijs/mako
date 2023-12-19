const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/math.ts", "function cube\\("),
  "should have function cube"
);
assert.doesNotMatch(
  content,
  moduleReg("src/math.ts", "function square\\("),
  "should not have function square"
);
