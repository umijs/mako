const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

// check files
assert.match(names, /person.(.*).svg/, "should have person.svg");

// check content
assert(content.includes(`new URL(__mako_require__.publicPath + "person.`), 'new URL() should be replaced');
assert(content.includes(`document.baseURI || self.location.href);`), 'import.meta.url should be replaced');
