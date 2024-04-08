const assert = require('assert');
const { parseBuildResult, moduleReg } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

assert.deepEqual(
  Object.keys(files).sort(),
  ['index.js', 'index.js.map'],
  'should only contain index.js and index.js.map',
);
