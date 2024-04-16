const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

require("./dist/index.js");


expect(files["index.js"]).not.toContain(moduleDefinitionOf("global.css"))
