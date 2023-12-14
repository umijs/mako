const assert = require("assert");

const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files["index.css"].includes(`flex: 1 1;`),
  "flex bugs is fixed"
);
