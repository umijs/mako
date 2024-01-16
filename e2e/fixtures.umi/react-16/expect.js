const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let content = files["umi.js"];

assert(content.includes(`_react.default.createElement(`), "should use classical runtime since react is 16");
