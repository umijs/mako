const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

// require esm 时不能被 tree shaking
assert(content.includes(`"src/b.ts":`), "should have src/b.ts module define");
assert(content.includes(`"src/c.ts":`), "should have src/c.ts module define");
assert(content.includes(`"src/d.ts":`), "should have src/d.ts module define");
