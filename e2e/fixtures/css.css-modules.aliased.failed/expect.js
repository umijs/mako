const assert = require("assert");

const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files["index.js"].includes(`index.css?asmodule`),
  "css module should work behind alias"
);
