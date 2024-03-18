const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

// check files
assert.match(names, /bigfish-log.(.*).png/, "should have bigfish-log.png");
assert.doesNotMatch(names, /umi-logo.(.*).png/, "should not have umi-logo.png");
assert.doesNotMatch(names, /index.css/, "should not have index.css");

// check content
assert(
  content.includes(`__mako_require__("src/a.css")`, `should work`),
);
assert(
  content.includes(`__mako_require__("src/b.css")`, `should support deps in css`),
);
assert(
  content.includes(`@import "//c";`, `should keep remote imports`),
);
