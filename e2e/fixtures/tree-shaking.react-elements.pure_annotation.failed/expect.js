const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

// TODO
assert(!content.includes("SHOULD_REMOVE"), `should not have REMOVE`);
