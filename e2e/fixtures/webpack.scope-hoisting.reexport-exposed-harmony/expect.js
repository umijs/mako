const {
	injectSimpleJest,
	parseBuildResult,
	moduleDefinitionOf,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();

require("./dist/index.js");


xit("should concatenate with b.js", function () {
  expect(files["index.js"]).no.toContain("b.js");
});