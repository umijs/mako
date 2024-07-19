const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files["index.css"].trim() ===
    `
@media (min-width: 500px) {
  .container {
    padding-top: 0.8rem;
  }
}
`.trim(),
);
