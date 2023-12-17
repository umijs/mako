const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  !content.includes(`require("node_modules/demo-pkg2/index.js")`),
  "should not have require for demo-pkg2"
);
