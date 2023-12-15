const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = trim(files['index.js']);

assert.match(content, /ҵ������/, "build success and output unknown character");
