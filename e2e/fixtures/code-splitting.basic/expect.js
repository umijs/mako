const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert("vendors-async.js" in files, "should have vendors-async.js");
