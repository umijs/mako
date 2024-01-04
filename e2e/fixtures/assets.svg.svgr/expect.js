const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

// check files
assert.match(names, /person.(.*).svg/, "should have person.svg");

// check content
assert.match(
  content,
  moduleReg(
    "src/assets/person.svg",
    '__mako_require__.d(exports, "ReactComponent", {',
    true
  ),
  "person.svg's content is not correct"
);
assert.match(
  content,
  moduleReg(
    "src/assets/person.svg",
    'const SvgComponent = (props)=>(0, _jsxdevruntime.jsxDEV)("svg", {',
    true
  ),
  "person.svg's content is not correct"
);
