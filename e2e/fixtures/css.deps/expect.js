const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.css"];

assert(
  content.includes(`@import "http://should-keep";`),
  "should keep http:// deps"
);
assert(
  content.includes(`@import "https://should-keep";`),
  "should keep https:// deps"
);
assert(
  content.includes(`@import "//should-keep";`),
  "should keep // deps"
);
assert(content.includes(`.bar {`), "should support non ./ prefix relative dep");
assert(content.includes(`.foo {`), "should support deps starts with ./");
assert(content.includes(`.foo-image {\n  background: url("data:image/png;`), "should support url image deps non ./ prefix relative dep");
assert(content.includes(`.bar-image {\n  background: url("data:image/png;`), "should support url image deps starts with ./");
