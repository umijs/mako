const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const jsFilesCount = Object.keys(files).filter(f => f.endsWith('.js')).length;

assert.equal(
  jsFilesCount,
  1,
  'should output one js file',
);
