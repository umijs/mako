const assert = require('assert');
const { parseBuildResult } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const content = files['index.js'];

assert(content.includes(`_export_star: function() {`), 'should have _export_star helper');
assert(
  content.includes(`_interop_require_wildcard: function() {`),
  'should have _interop_require_wildcard helper',
);
assert(
  content.includes(`_interop_require_default: function() {`),
  'should have _interop_require_default helper',
);
