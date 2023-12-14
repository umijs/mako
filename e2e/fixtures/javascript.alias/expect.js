const assert = require("assert");

const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg("src/foo/index.js", 'console.log\\("foo"\\)'),
  "should have src/foo/index.js"
);

assert.match(
  content,
  moduleReg("src/bar/index.js", 'console.log\\("bar"\\)'),
  "should have src/bar/index.js"
);

assert.match(
  content,
  moduleReg("src/zoo_hoo.ts", 'console.log\\("zoo_hoo"\\)'),
  "should have src/zoo_hoo.ts"
);
