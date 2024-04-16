const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

require("./dist/index.js");


// assert require cjs resolved
expect(files["index.js"]).toContain(`__mako_require__("cjs.js")`)
expect(files["index.js"]).not.toContain(`__mako_require__("./cjs")`)

expect(files["index.js"]).not.toContain(moduleDefinitionOf("b.js"))