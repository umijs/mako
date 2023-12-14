const assert = require("assert");

const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/index.ts", 'console.log(__mako_require__("../../../node_modules', true),
  "should transform native require"
);

assert.match(
  content,
  moduleReg("src/index.ts", '\\(\\)=>\\{\\s+console.log\\(__mako_require__\\("../../../node_modules'),
  "should transform nested native require"
);

assert.match(
  content,
  moduleReg("src/index.ts", 'require\\(1\\);\\s+\\}\\)\\(\\);'),
  "should keep nested fake require with args"
);

assert.match(
  content,
  moduleReg("src/index.ts", 'require\\(\\);\\s+\\}\\)\\(\\);'),
  "should keep nested fake require without args"
);
