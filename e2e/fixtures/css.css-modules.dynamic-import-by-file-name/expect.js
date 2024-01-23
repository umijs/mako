const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let cssFile = Object.keys(files).filter(k=> k.endsWith(".css"))[0];
assert(files[cssFile].match(/\.container-\S*/), 'should contains postfixed classname')

assert(
  trim(files["index.js"]).includes(`__mako_require__.ensure("src/index.module.css?asmodule")`),
  "should find ensure chunk"
);

assert(
  files["index.js"].includes(`__mako_require__.bind(__mako_require__, "src/index.module.css?asmodule")`),
  "should find reuquire module"
);
