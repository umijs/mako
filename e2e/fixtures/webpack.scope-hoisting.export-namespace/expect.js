const {
	injectSimpleJest,
	parseBuildResult
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

require("./dist/index.js");

expect(files['index.js']).not.toContain(`"ns1.js": function`);
expect(files['index.js']).not.toContain(`"ns2.js": function`);

