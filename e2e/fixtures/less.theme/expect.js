const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = trim(files['index.css']);

assert.match(content, /font-size:12px;/, "font-size is not expected");
assert.match(content, /font-family:PingFangSC,/, "font-family is not expected");
