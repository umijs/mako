const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg(
    "src/index.tsx",
    `__mako_require__._async(module, async (handleAsyncDeps, asyncResult)=>{
\\s*async function af() {}
\\s*await af();
\\s*asyncResult();
\\s*}, 1);`,
    true
  ),
  "should have __mako_require__._async"
);
