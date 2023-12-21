const assert = require('assert');
const { parseBuildResult, moduleReg } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

assert.deepEqual(
  Object.keys(files).sort(),
  ['index.js', 'index.js.map'],
  'no extract chunk generated',
);

assert(
  files['index.js'].includes(`"node_modules/foo/index.js":`),
  'dynamic imported module(foo) not existss',
);
assert(
  files['index.js'].includes(`"src/foo.js":`),
  'dynamic imported module(./foo) not exists',
);
