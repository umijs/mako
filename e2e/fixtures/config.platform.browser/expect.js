const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  /console\.log\('filename', '\/index\.js'\)/,
  "should transform __filename"
)
assert.match(
  content,
  /console\.log\('dirname', '\/'\)/,
  "should transform __dirname"
)
