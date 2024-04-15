const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

require("./dist/index.js");

expect(files["index.js"]).not.toContain(moduleDefinitionOf("b.js"))
expect(files["index.js"]).toContain(moduleDefinitionOf("@swc/helpers/_/_interop_require_default"))