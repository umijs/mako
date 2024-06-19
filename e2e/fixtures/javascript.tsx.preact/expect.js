const { parseBuildResult, injectSimpleJest } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
injectSimpleJest();


require("./dist/index")


