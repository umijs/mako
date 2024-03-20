const assert = require("assert");
const { parseBuildResult, trim, moduleReg, injectSimpleJest } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

injectSimpleJest();
const content = files["index.js"];

assert.match(
  content,
  moduleReg(
    "src/index.tsx",
    `__mako_require__._async(module, async (handleAsyncDeps, asyncResult)=>{`, true),
  "should have __mako_require__._async"
);

require("./dist/index.js");
