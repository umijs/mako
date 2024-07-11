const assert = require("assert");
const { parseBuildResult, moduleReg, injectSimpleJest } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest()
const content = files["index.js"];
assert.match(
  content,
  moduleReg("index.ts", `use strict`, true),
  "entry should have use strict directive"
);
require("./dist/index.js");

