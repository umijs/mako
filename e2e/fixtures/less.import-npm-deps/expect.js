const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert.match(
  trim(files["index.css"]),
  /.foo{color:red;}/,
  "should support import npm deps without ~ prefix"
);

assert.match(
  trim(files["index.css"]),
  /.bar{color:red;}/,
  "should support import npm deps with ~ prefix"
);
