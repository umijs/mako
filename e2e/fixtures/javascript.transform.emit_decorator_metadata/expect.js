const assert = require("assert");
const { parseBuildResult, trim, moduleReg, injectSimpleJest } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];
assert(
  content.includes(`_ts_metadata._("design:type", Function),`),
  `emitDecoratorMetadata works`,
);
