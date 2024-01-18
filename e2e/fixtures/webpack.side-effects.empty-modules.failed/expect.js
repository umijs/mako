const { injectSimpleJest, parseBuildResult } = require("../../../scripts/test-utils") 
const { files } = parseBuildResult(__dirname);
injectSimpleJest()


expect(files["index.js"]).not.toContain("/cjs.js")
expect(files["index.js"]).not.toContain("/expect.js")
expect(files["index.js"]).not.toContain("/index.js")
expect(files["index.js"]).not.toContain("/module.js")
expect(files["index.js"]).not.toContain("/pure.js")
expect(files["index.js"]).not.toContain("/referenced.js")
expect(files["index.js"]).not.toContain("/side-referenced.js")
expect(files["index.js"]).not.toContain("/side.js")



expect(files["index.js"]).not.toContain()
require("./dist/index.js");
