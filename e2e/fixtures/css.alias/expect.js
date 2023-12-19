const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert.match(trim(files['index.css']), /.foo{color:red;}/, "css content is not expected");
