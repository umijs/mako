const { injectSimpleJest, parseBuildResult } = require("../../../scripts/test-utils") 
const { files } = parseBuildResult(__dirname);
injectSimpleJest()

expect(files["index.js"]).not.toContain("src/module/A/A.js")
require("./dist/index.js");
