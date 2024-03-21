const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  Object.keys(files).find((file) => /xx.[0-9a-zA-z]*.schema/.test(file)),
  "add file of unsupported mime to assets"
);

const content = files["index.js"];
assert(
  content.includes(`"src/xx.schema?a=1":`),
  "assets with query should be handled correctly"
);
