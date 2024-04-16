const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

require("./dist/index.js");

expect(files['index.js']).not.toContain(moduleDefinitionOf("file1.js"));
expect(files['index.js']).not.toContain(moduleDefinitionOf("file2.js"));
expect(files['index.js']).not.toContain(moduleDefinitionOf("file3.js"));
expect(files['index.js']).not.toContain(moduleDefinitionOf("file4.js"));
expect(files['index.js']).not.toContain(moduleDefinitionOf("module.js"));
