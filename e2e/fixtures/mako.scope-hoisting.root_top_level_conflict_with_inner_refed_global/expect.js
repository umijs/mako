const {
  injectSimpleJest,
  parseBuildResult,
  moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

expect(files["index.js"]).not.toContain(moduleDefinitionOf("inner-next.js"));

require("./dist/index.js");
