const {
	injectSimpleJest,
	parseBuildResult
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

const content = files["index.js"];

expect(content).toMatch(/__mako_module_js_0_0/);
expect(content).toMatch(/__mako_module_js_0_1/);
expect(content).toMatch(/__mako_module_js_0_2/);
expect(content).toMatch(/__mako_module_js_0_3/);
expect(content).toMatch(/__mako_module_js_0_4/);
expect(content).not.toMatch(/__mako_module_js_0_5/);

require("./dist/index.js");
