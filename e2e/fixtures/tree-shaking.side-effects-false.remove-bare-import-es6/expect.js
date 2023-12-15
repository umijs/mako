const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

// TODO: esm 的也没删？
// assert(!content.includes("REMOVE"), `should not have REMOVE`);
