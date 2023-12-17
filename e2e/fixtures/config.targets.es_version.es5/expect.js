const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/index.tsx", "var abstract = 1;"),
  "reserved word should not be reserved"
);
assert.match(
  content,
  moduleReg("src/index.tsx", "var A = function A\\(\\) \\{"),
  "class should be converted"
);
assert.match(
  content,
  moduleReg("src/index.tsx", "var b = \\(0, _object_spread._\\)\\(\\{\\}, \\{"),
  "spread operator should be converted"
);
assert.match(
  content,
  moduleReg("src/index.tsx", 'var c = "".concat\\(a, "_1"\\);'),
  "template literal should be converted"
);
