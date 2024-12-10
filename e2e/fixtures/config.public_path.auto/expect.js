const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  /scriptUrl = document.currentScript.src.*requireModule.publicPath = scriptUrl/s,
  "requireModule.publicPath not correct"
);
