const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.doesNotMatch(
  content,
  moduleReg("src/index.tsx", "const f = \\("),
  "should not have `const f`"
);
assert.doesNotMatch(
  content,
  moduleReg("src/index.tsx", "function default\\("),
  "should not have `function default`"
);
assert.match(
  content,
  moduleReg("src/index.tsx", "var f = function\\("),
  "should have `var f`"
);
