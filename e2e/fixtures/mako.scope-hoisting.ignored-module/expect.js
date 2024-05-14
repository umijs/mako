const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

// ignored module should not be concatenated, so keep the definition
expect(files["index.js"]).toContain(moduleDefinitionOf("node_modules/pkg/index.js"));

require("./dist/index.js");
