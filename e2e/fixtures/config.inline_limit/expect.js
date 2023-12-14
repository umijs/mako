const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

// check files
assert.match(names, /bigfish-log.(.*).png/, "should have bigfish-log.png");
assert.doesNotMatch(names, /umi-logo.(.*).png/, "should not have umi-logo.png");

// check content
assert.match(
  content,
  moduleReg(
    "src/assets/bigfish-log.png",
    "module.exports = `\\${__mako_require__.publicPath}bigfish-log.(.*).png`;"
  ),
  "bigfish-log.png's content is not correct"
);
assert.match(
  content,
  moduleReg(
    "src/assets/umi-logo.png",
    'module.exports = "data:image/png;base64,'
  ),
  "umi-logo.png's content is not correct"
);
