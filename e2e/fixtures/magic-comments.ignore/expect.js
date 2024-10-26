const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let index = files["index.js"];
assert(index.includes(`import(/* webpackIgnore: true */ "./foo");`), "should include foo");
assert(index.includes(`import(/* makoIgnore: true */ "./bar");`), "should include bar");
