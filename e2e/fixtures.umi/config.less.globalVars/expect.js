const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let content = files["pages_index_tsx-async.css"];
content = content.replace(/\s/g, "");

assert(content.includes(`color:red;`), "should available less.globalVars");
