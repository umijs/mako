const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);


const content = files["index.js"];

assert.match(
  content,
  moduleReg("index.tsx", "var h = h_1", true),
  "should keep require preact programa h",
  true
);

assert.match(
  content,
  moduleReg("index.tsx", "h(\"div\"", true),
  "should use preact programa h",
  true
);


