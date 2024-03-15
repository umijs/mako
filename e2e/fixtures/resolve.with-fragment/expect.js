const { parseBuildResult } = require("../../../scripts/test-utils");
const { distDir } = parseBuildResult(__dirname);

require(path.join(distDir, 'index.js'));

