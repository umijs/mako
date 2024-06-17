const assert = require("assert");
const {
  parseBuildResult,
  moduleReg,
  injectSimpleJest,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
const content = files["index.js"];

injectSimpleJest();
require("./dist/index.js");
