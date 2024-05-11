const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

expect(files["index.js"]).toContain(moduleDefinitionOf("node_modules/pkg/index.js"));

require("./dist/index.js");
