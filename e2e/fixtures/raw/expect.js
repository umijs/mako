const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  content.includes(
    'module.exports = "const Hello = \\"Hello\\";\\nconst World = `World`;\\n"'
  ),
  "support convert js"
);

assert(
  content.includes("h1 {\\n    background-size: 20px 20px;\\n}\\n"),
  "support convert css"
);
