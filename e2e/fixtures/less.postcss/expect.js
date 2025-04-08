const assert = require("assert");

const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert.match(
  files["index.css"],
  new RegExp(`.foo .bar {
  width: 100vw;
}`),
  "less width is not expected"
);

assert.match(
  files["index.css"],
  new RegExp(`.a .b {
  width: 100vw;
}`),
  "css width is not expected"
);

