const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert("foo.js" in files, "assets files not copy");
