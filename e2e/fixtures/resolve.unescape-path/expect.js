const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(content.includes(`console.log('中');`), "should contain 中");
assert(content.includes(`console.log('a b');`), "should contain a b");
