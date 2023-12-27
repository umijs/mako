const assert = require('assert');
const { parseBuildResult } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const content = files['index.js'];

assert(!content.includes('dep/index.js'), `dep/index.js should be skipped`);
assert(
  content.includes('console.log(_dep.z);'),
  `access field changed to exported name`,
);
