const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  !content.includes('import.meta.path'),
  "should have not have import.meta in entry.js module"
);
