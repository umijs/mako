const assert = require("assert");
const { parseBuildResult, trim, injectSimpleJest } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

injectSimpleJest();

require('./dist/index.js');
