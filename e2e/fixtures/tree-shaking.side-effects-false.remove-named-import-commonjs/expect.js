const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(!content.includes("node_modules/demo-pkg/index.js"), `should not have node_modules/demo-pkg/index.js`);
