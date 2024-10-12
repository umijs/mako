const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");

// check files
assert.match(names, /abc.(.*).webp/, "should have origin webp");
