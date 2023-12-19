const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const fs = require("fs");
const { files } = parseBuildResult(__dirname);

const test = async () => {
  assert("index.js" in files, "should have file: index.js");
  assert(!("index.js.map" in files), "should not have file: index.js.map");

  const indexContent = fs.readFileSync(
    "e2e/fixtures/config.devtool.inline-source-map/dist/index.js"
  );
  assert(
    indexContent.indexOf("//# sourceMappingURL=data:application/json"),
    "should have source map in index.js"
  );
};

module.exports = test;
