const {
  injectSimpleJest,
  parseBuildResult,
  moduleDefinitionOf,
} = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

require('./dist/index.js');

it.skip('b.js should be concatenate', () => {
  expect(files['index.js']).not.toContain(moduleDefinitionOf('b.js'));
});
