const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  !content.includes(`require("a")`) &&
    !content.includes(`require("b")`) &&
    !content.includes(`require("c")`),
  `should not have require a,b,c`
);
