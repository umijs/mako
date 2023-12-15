const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/index.tsx", "const global ="),
  "should have polyfill module: global"
);

assert.match(
  content,
  moduleReg("src/index.tsx", "const process ="),
  "should have polyfill module: process"
);
