const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(!content.includes("dep/index.js"), `dep/index.js should be skipped`);
assert(!content.includes("dep/y.js"), `unused namespace should tree-shaken`);

assert(content.includes("dep/x.js"), `should keep namespace exported module`);
assert(content.includes("dep/a.js"), `should keep namespace exported module's dep`);

assert(content.includes("console.log(_x);"), `access field changed to exported name`);
