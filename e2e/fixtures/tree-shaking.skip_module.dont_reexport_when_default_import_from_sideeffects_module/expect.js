const assert = require('assert');
const { parseBuildResult } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const content = files['index.js'];

assert(
  content.includes('node_modules/pure/index.js'),
  `should not skip pure module when it use default import`,
);
assert(
  content.includes('node_modules/side_effects/index.js'),
  `should keep all side effects modules`,
);
assert(
  content.includes('node_modules/side_effects/dep.js'),
  `should keep all side effects modules`,
);
