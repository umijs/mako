const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let content = files["p__index-async.css"];
content = content.replace(/\s/g, "");

assert(content.includes(`color:blue;`), "should prefer less.modifyVars than config.theme");
