const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

assert(content.includes(`Object.defineProperty(exports, "foo"`), "should have foo exports in 1.ts");
assert(!content.includes(`Object.defineProperty(exports, "bar"`), "should have bar exports in 1.ts");
assert(!content.includes(`Object.defineProperty(exports, "zoo"`), "should have zoo exports in 1.ts");
assert(!content.includes(`"4.ts":`), "should not have 4.ts module define");
