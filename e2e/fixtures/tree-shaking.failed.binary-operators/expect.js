const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

// TODO: 1\ remove 多了 2\ keep 少了
// assert(!content.includes("REMOVE"), `should not have REMOVE`);
