const assert = require("assert");
const path = require("path");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { distDir } = parseBuildResult(__dirname);

const ret = require(path.join(distDir, 'index.js')).default();
assert(ret === 'abc', 'Run dist/index.js does not throw error.');
