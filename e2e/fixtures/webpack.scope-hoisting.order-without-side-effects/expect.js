const {
	injectSimpleJest,
	parseBuildResult
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

// expect(files['index.js']).toContain("ROOT MODULE: ./index.js");
// expect(files['index.js']).toContain("CONCATENATED MODULE: ./a.js");
// expect(files['index.js']).toContain("CONCATENATED MODULE: ./b.js");
// expect(files['index.js']).toContain("CONCATENATED MODULE: ./tracker.js");

require("./dist/index.js");
