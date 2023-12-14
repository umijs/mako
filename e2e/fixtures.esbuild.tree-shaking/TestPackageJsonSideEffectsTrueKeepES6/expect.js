const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  content.includes(`"node_modules/demo-pkg/index.js":`),
  "should have demo-pkg/index.js module define"
);
