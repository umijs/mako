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
  files['index.js'].includes(
    `Promise.resolve().then(()=>__mako_require__("node_modules/foo/index.js"))`,
  ),
  'require(foo) statement not found',
);

assert(
  files['index.js'].includes(`"src/foo.js":`),
  'dynamic imported module(./foo) not exists',
);
assert(
  files['index.js'].includes(
    `Promise.resolve().then(()=>__mako_require__("src/foo.js"))`,
  ),
  'require(./foo) statement not found',
);
