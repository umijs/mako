const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

// No module is concatenated, all are kept
expect(files["index.js"]).toContain(moduleDefinitionOf("index.js"));
expect(files["index.js"]).toContain(moduleDefinitionOf("inner.js"));
expect(files["index.js"]).toContain(moduleDefinitionOf("inner2.js"));
expect(files["index.js"]).toContain(moduleDefinitionOf("async.js"));

require("./dist/index.js");
