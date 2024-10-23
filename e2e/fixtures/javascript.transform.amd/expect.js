const {parseBuildResult, injectSimpleJest} = require("../../../scripts/test-utils");
const {distDir} = parseBuildResult(__dirname);

injectSimpleJest()
require('./dist/index.js');

