const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  /requireModule.publicPath = "http:\/\/127.0.0.1:8001\/"/,
  "requireModule.publicPath not correct"
);
