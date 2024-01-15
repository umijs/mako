const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

assert(
  !content.includes(`src/index.css`),
  "should remove resolved css file"
);

assert(
  !content.includes(`myCss`),
  "should remove aliased css file"
);
