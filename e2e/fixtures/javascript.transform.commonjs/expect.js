const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];
assert.match(
  content,
  moduleReg("index.ts", `use strict`, true),
  "entry should have use strict directive"
);
