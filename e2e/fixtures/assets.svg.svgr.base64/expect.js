const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/assets/person.svg", "ReactComponent: function() {", true),
  "person.svg's content should have ReactComponent"
);

assert.match(
  content,
  moduleReg(
    "src/assets/person.svg",
    'var _default = "data:image/svg\\+xml;base64,'
  ),
  "person.svg's data should be base64"
);
