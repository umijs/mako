const assert = require("assert");
const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(files, "should have dist");
