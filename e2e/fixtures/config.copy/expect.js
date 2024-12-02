const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

// Test original string pattern (copies to root)
assert("foo.js" in files, "assets files not copied (string pattern)");

// Test new from/to pattern (copies to assets-from-to directory)
assert("assets-from-to/foo.js" in files, "assets files not copied to correct location (from/to pattern)");
