const assert = require("assert");
const {
  parseBuildResult,
  injectSimpleJest,
  moduleReg,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

injectSimpleJest();
const content = files["index.js"];

expect(content).toContain("shouldKeep");
expect(content).not.toContain("shouldNotKeep");
