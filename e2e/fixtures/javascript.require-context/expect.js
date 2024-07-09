const {
  injectSimpleJest,
  parseBuildResult,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

injectSimpleJest();

require("./dist/index");

expect(
  Object.keys(files).filter((f) => f.endsWith(".js")).length,
).toBeGreaterThan(2);
