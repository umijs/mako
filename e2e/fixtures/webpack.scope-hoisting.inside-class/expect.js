const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();


expect(files['index.js']).not.toContain(moduleDefinitionOf("firtst.js"));
expect(files['index.js']).not.toContain(moduleDefinitionOf("second.js"));

require("./dist/index.js");
