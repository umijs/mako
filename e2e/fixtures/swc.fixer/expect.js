const assert = require("assert");

const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("index.tsx", "a.b()", true),
  "fixer should run before simplifier to avoid this is undefined when callee (a.b)()",
);