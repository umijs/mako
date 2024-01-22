const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

module.exports = async () => {
  const result = await require('./dist').bar();
  assert(result === 'foo_bar', `should support code splitting`);
};
