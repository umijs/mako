const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();


// a.js treated as external
expect(files["index.js"]).toContain(moduleDefinitionOf("a.js"))
