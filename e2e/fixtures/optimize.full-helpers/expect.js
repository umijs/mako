const assert = require('assert');
const { parseBuildResult } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const content = files['index.js'];

assert(content.includes(`"@swc/helpers/_/_export_star":`), 'should have _export_star helper');
assert(
  content.includes(`"@swc/helpers/_/_interop_require_wildcard":`),
  'should have _interop_require_wildcard helper',
);
assert(
  content.includes(`"@swc/helpers/_/_interop_require_default":`),
  'should have _interop_require_default helper',
);
