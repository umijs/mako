const assert = require('assert');
const { parseBuildResult, trim, moduleReg } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const content = files['index.js'];

assert.doesNotMatch(
  content,
  moduleReg('src/dep.ts', 'declareWithOutSideEffects'),
  'should not have side effects free stmt',
);

assert.deepStrictEqual(content.match(/declareWithSideEffects\d/g), [
  'declareWithSideEffects1',
  'declareWithSideEffects2',
  'declareWithSideEffects3',
]);
