const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/index.tsx", "const a = 100;"),
  "exponentiation should be calculated"
);

assert.match(
  content,
  moduleReg("src/index.tsx", "_abc = \\(0, _async_to_generator._\\)"),
  "async function should be converted"
);
