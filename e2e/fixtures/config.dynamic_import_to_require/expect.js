const assert = require('assert');
const { parseBuildResult } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const index = files["index.js"];

require("./dist/index")





assert.deepEqual(
  Object.keys(files).sort(),
  ['index.js', 'index.js.map'],
  'no extract chunk generated',
);



assert(
  index.includes(`"node_modules/foo/index.js":`),
  'dynamic imported module(foo) not existss',
);
assert(
  index.includes(
    `Promise.resolve().then(()=>__mako_require__("node_modules/foo/index.js")).then(interop)`,
  ),
  'require(foo) statement not found',
);

assert(
  index.includes(`"src/foo.js":`),
  'dynamic imported module(./foo) not exists',
);
assert(
  index.includes(
    ` Promise.resolve().then(()=>__mako_require__("src/foo.js")).then(interop)`,
  ),
  'require(./foo) statement not found',
);
assert(
  index.includes(
    `var interop = __mako_require__("@swc/helpers/_/_interop_require_wildcard")._;`,
  ),
  'await import need var interop',
);
assert(
  index.includes(
    `Promise.resolve().then(()=>__mako_require__("src/lazy.ts")).then(interop);`,
  ),
  'await import result need interop',
);

