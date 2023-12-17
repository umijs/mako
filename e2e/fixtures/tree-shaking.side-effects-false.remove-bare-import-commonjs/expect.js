const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

// TODO: 暂不支持 cjs 模块的 tree-shaking
// assert(!content.includes("REMOVE"), `should not have REMOVE`);
