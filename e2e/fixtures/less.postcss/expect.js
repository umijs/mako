const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert.match(
  trim(files["index.css"]),
  /.foo{width:100vw;}/,
  "width is not expected"
);
