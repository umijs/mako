const assert = require("assert");

const { parseBuildResult, trim, moduleReg, injectSimpleJest } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

injectSimpleJest();

expect(content.length).toBeGreaterThan(0);


