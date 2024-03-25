const {
	injectSimpleJest,
	parseBuildResult
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

require("./dist/index.js");

expect(files['index.js']).not.toContain("a.js");
expect(files['index.js']).not.toContain("b.js");

