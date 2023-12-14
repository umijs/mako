const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

// webpack 中 cjs 的 bbb 会被标记为 unused
assert(
  content.includes(`"src/cjs.js":`),
  "should have src/cjs.js module define"
);
