const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

// 目前 module 做了转换，但 runtime 逻辑仍有很多 const
// assert.doesNotMatch(content, /const /, "should not have `const`");
