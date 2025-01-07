const assert = require("assert");
const { parseBuildResult  } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.css"];

assert(
  content.includes(`
body {
  color: red;
}
body {
  color: blue;
}
  `.trim()),
  "css should be right after optimization"
);
