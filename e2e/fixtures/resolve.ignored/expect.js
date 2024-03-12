const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { distDir } = parseBuildResult(__dirname);

const ret = require(path.join(distDir, 'index.js'));

assert.deepEqual(
  ret,
  {},
  `ignored module should compile to empty es module`
);
