const { injectSimpleJest, parseBuildResult } = require("../../../scripts/test-utils") 
const { files } = parseBuildResult(__dirname);
injectSimpleJest()

expect(files["index.js"]).toContain("index.js")
expect(files["index.js"]).toContain("a.js")
expect(files["index.js"]).toContain("b.js")
expect(files["index.js"]).toContain("node_modules/base/index.js")
expect(files["index.js"]).toContain("node_modules/tslib.js")
expect(files["index.js"]).not.toContain("reexport.js")


require("./dist/index.js");
