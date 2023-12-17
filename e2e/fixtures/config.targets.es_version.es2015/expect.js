const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/index.tsx", "const abstract = 1;"),
  "reserved word should be reserved"
);
assert.match(
  content,
  moduleReg("src/index.tsx", "constructor\\(\\)\\{"),
  "class should be reserved"
);
assert.match(
  content,
  moduleReg("src/index.tsx", "const c = \\`\\$\\{a\\}_1`;"),
  "template literal should be reserved"
);

assert.match(
  content,
  moduleReg("src/index.tsx", "const x = Math.pow\\(10, 2\\);"),
  "exponentiation should be converted"
);
