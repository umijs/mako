const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

assert(content.includes(`should_keep()`), "should have side effects statement");
assert(
  !content.includes(`should_not_keep()`),
  "should shake away unused export",
);
