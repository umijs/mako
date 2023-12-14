const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];
const cssContent = files["index.css"];

assert.match(
  content,
  /"node_modules\/foo\/es\/button\/index.js"/,
  "js should have foo/es/button"
);
assert.match(cssContent, /\.foo-btn/, "css should have `.foo-btn`");
