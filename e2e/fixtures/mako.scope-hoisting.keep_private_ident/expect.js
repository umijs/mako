const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

// expect(files["index.js"]).toContain(`ROOT MODULE: ./index.js`);
expect(files["index.js"]).not.toContain(moduleDefinitionOf("module_fn.js"));

require("./dist/index.js");
