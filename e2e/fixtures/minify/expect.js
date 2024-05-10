const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  content.includes(`console.log("\\u4E2D\\u6587");`),
  `Chinese characters should be unicode escaped`
)
