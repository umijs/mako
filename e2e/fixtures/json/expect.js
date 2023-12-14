const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/example.json", 'module.exports = {\\s*"foo": "bar"\\s*};'),
  "should have example.json module"
);
