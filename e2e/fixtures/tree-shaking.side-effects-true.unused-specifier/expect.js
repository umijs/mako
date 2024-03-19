const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg(
    "src/index.js",
    `__mako_require__\\("src/a.js"\\);[\\s\\S]*__mako_require__\\("src/b.js"\\);[\\s\\S]*__mako_require__\\("src/c.js"\\);`
  ),
  `should remove namespace specifier`
);
