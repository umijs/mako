const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(!content.includes("dep/dep1.js"), `should not contains reexport module`);
assert(!content.includes("dep/dep2.js"), `should not contains reexport module`);
assert(!content.includes("dep/dep3.js"), `should not contains reexport module`);
assert(!content.includes("dep/dep4.js"), `should not contains reexport module`);

assert(content.includes("dep5.e"), `should change access field from .a to .z`);
