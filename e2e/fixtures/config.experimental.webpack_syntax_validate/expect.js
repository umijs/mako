const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];
assert(content.includes(`__webpack_require__('abc');`), 'experimental.webpackSyntaxValidate should work');
