const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  content.includes(
    `/*./node_modules/ignored/index.js*/ "node_modules/ignored/index.js": function(module, exports, __mako_require__) {}`
  ),
  `ignored module should compile to empty module`
);
