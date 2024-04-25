const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  !content.includes(`requireModule.chunkEnsures.css`),
  `requireModule.chunkEnsures.css should not be included in the output`
);
