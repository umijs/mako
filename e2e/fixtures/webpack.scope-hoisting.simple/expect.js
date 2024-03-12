const {
	injectSimpleJest,
	parseBuildResult
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

expect(files["index.js"]).toContain(`ROOT MODULE: ./index.js`);
expect(files["index.js"]).toContain('_simple_module_js_0');

require("./dist/index.js");
