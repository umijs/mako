const { parseBuildResult, injectSimpleJest } = require("../../../scripts/test-utils");
const { distDir } = parseBuildResult(__dirname);
const path = require("path");

injectSimpleJest()
require(path.join(distDir, 'index.js'));


