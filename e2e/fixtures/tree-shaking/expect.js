const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

assert(content.includes(`Object.defineProperty(exports, "foo", {`), "should has foo exports");
assert(!content.includes(`Object.defineProperty(exports, "bar", {`), "should not has bar exports");
assert(!content.includes(`Object.defineProperty(exports, "zoo", {`), "should not has zoo exports");
assert(!content.includes(`const foo = 2;`), "should not has foo = 2, it's been tree-shaked");
assert(!content.includes(`const zoo;`), "should not has zoo = 2, it's been tree-shaked");
