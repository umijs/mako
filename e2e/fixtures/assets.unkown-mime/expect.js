const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  Object.keys(files).find((file) => /xx.[0-9a-zA-z]*.schema/.test(file)),
  "add file of unsupported mime to assets"
);

const content = files["index.js"];
assert.match(
  content,
  moduleReg("src/xx.schema", "\\${__mako_require__.publicPath}xx", true),
  "person.svg's content should have ReactComponent"
);
