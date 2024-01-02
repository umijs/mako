const assert = require('assert');
const { parseBuildResult } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const content = files['index.js'];

assert(
  content.includes('src/dep/index.js'),
  `dep/index.js should keep in chunk`,
);
assert(content.includes('src/dep/dep.js'), `dep/dep.js should keep in chunk`);
