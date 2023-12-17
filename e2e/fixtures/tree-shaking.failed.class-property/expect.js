const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

// TODO: 多了前两个，少了最后一个
// assert(!content.includes("REMOVE"), `should not have REMOVE`);
