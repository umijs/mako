const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert("index.js" in files, "should have file: index.js");
assert(!("index.js.map" in files), "should not have file: index.js.map");
