const assert = require("assert");
const { parseBuildResult, string2RegExp } = require("../../../scripts/test-utils");
const { isRegExp } = require("util/types");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  new RegExp(`Cannot find module 'antd'`),
  "should not have antd module definition",
);
