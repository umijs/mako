const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(content.includes(`__mako_require__("$$IGNORED$$")`), `should contain __mako_require__("$$IGNORED$$")`);
