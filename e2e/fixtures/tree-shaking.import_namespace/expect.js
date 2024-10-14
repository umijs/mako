const assert = require("assert");
const {
  parseBuildResult,
  injectSimpleJest,
  moduleReg,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

injectSimpleJest();
const content = files["index.js"];

expect(content).toContain("shouldKeep1");
expect(content).toContain("shouldKeep2");
expect(content).not.toContain("shouldNotKeep");
