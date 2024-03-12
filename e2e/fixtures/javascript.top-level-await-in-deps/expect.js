const assert = require("assert");
const { parseBuildResult, trim, moduleReg, injectSimpleJest } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

const content = files["index.js"];
assert.match(
  content,
  moduleReg("src/index.tsx", `__mako_require__._async.+`, true),
  "entry should have __mako_require__._async"
);

assert.match(
  content,
  moduleReg("src/b.ts", `__mako_require__._async.+`, true),
  "top level await module should have __mako_require__._async"
);

require("./dist/index.js");