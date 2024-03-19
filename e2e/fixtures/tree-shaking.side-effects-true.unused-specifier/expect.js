const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const contentA = files["entry-a.js"];
const contentB = files["entry-b.js"];
const contentC = files["entry-c.js"];

assert.doesNotMatch(
  contentA,
  moduleReg(
    "src/entry-a.js",
    `var _a`
  ),
  `should remove default specifier`
);

assert.match(
  contentA,
  moduleReg(
    "src/entry-a.js",
    `__mako_require__\\("src/a.js"\\);`
  ),
  `should remove default specifier`
);

assert.doesNotMatch(
  contentB,
  moduleReg(
    "src/entry-b.js",
    `var _b`
  ),
  `should remove default specifier`
);

assert.match(
  contentB,
  moduleReg(
    "src/entry-b.js",
    `__mako_require__\\("src/b.js"\\);`
  ),
  `should remove namespace specifier`
);

assert.doesNotMatch(
  contentC,
  moduleReg(
    "src/entry-c.js",
    `var _c`
  ),
  `should remove default specifier`
);

assert.match(
  contentC,
  moduleReg(
    "src/entry-c.js",
    `__mako_require__\\("src/c.js"\\);`
  ),
  `should remove named specifier`
);
