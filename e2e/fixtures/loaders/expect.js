const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const index_js_content = files["index.js"];
const index_css_content = files["index.css"];

assert(index_js_content.includes('"foo": "json"'), "json loader");
assert(index_js_content.includes("var _default = MDXContent;"), "md loader");
assert(index_js_content.includes('"foo": "json5"'), "json5 loader");
assert(index_js_content.includes('"foo": "toml"'), "toml loader");
assert(index_js_content.includes('"$value": "foo"'), "xml loader");
assert(index_js_content.includes('"foo": "yaml"'), "yaml loader");
assert(index_css_content.includes(".foo {\n  color: red;\n}"), "css loader");
assert(index_css_content.includes('.jpg");\n}'), "big.jpg in css");
assert(
  index_css_content.includes('.big {\n  background: url("'),
  "small.png in css"
);
assert(
  index_js_content.includes('big.jpg": function('),
  "include big.jpg in js"
);
assert(
  index_js_content.includes('small.png": function('),
  "include small.png in js"
);
