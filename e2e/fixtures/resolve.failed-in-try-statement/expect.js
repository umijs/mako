const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  /Cannot find module '\.\/foo'/,
  "should support failed require('...') in try statement"
);

assert.match(
  content,
  /Cannot find module '\.\/bar'/,
  "should support failed exports.xxx = require('...') in try statement"
);

assert.match(
  content,
  /Cannot find module '\.\/hoo'/,
  "should support failed var x = require('...') in try statement"
);
