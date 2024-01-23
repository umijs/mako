const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let cssFile = Object.keys(files).filter(k=> k.endsWith(".css"))[0];
assert(files[cssFile].match(/\.container-\S*/), 'should contains postfixed classname')

assert(
  files["index.js"].includes(`"src/index.module.css?asmodule":`),
  "should tree css file as css module"
);

assert(
  files["index.js"].includes(`__mako_require__("src/index.module.css?asmodule")`),
  "should reuquire css module with query"
);
