const { parseBuildResult, injectSimpleJest } = require("../../../scripts/test-utils");
const { distDir } = parseBuildResult(__dirname);

injectSimpleJest()
require(path.join(distDir, 'index.js'));

