const {
  injectSimpleJest,
  parseBuildResult,
  moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

expect(files["index.js"]).not.toContain(moduleDefinitionOf("module.js"));
expect(files["index.js"]).not.toContain(moduleDefinitionOf("pure.js"));
expect(files["index.js"]).not.toContain(moduleDefinitionOf("referenced.js"));
expect(files["index.js"]).not.toContain(
  moduleDefinitionOf("side-referenced.js"),
);
expect(files["index.js"]).toContain(moduleDefinitionOf("side.js"));

require("./dist/index.js");
