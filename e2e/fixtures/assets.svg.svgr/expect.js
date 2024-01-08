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
    'Object.defineProperty(exports, "ReactComponent", {',
    true
  ),
  "person.svg's content is not correct"
);
assert(
  content.includes(`const SvgComponent = (props)=>/*#__PURE__*/ (0, _jsxdevruntime.jsxDEV)("svg", {`),
  "person.svg's content is not correct"
);
