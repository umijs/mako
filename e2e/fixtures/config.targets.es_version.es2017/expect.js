const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/index.tsx", "async function abc\\(\\) \\{\\}"),
  "async function should be reserved"
);

assert.match(
  content,
  moduleReg("src/index.tsx", "\\(0, _object_without_properties._\\)"),
  "object rest spread should be converted"
);
