const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  /requireModule.publicPath = \(typeof globalThis !== 'undefined' \? globalThis : self\).publicPath \|\| '\/';/,
  "requireModule.publicPath not correct"
);
