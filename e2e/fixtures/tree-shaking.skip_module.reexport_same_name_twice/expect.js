const assert = require('assert');
const { parseBuildResult } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const content = files['index.js'];

assert(!content.includes('src/dep/proxy.js'), `should skip middle files`);

assert(content.match(/v1:\s+function\(\) {\s+return _dep\.default;\s+}/), `shoule export v1 ref to _dep.default`)
assert(content.match(/v2:\s+function\(\) {\s+return _dep\.default;\s+}/), `shoule export v2 ref to _dep.default`)
