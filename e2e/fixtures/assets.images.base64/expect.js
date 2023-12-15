const assert = require("assert");
const {
  parseBuildResult,
  string2RegExp,
  moduleReg,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  string2RegExp('"src/assets/umi-logo.png"'),
  "should have umi-logo.png"
);
assert.match(
  content,
  moduleReg(
    "src/assets/umi-logo.png",
    'module.exports = "data:image/png;base64,'
  ),
  "umi-logo.png'data should be base64"
);
