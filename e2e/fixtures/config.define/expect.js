const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let content = files["index.js"];
content = content.replace(/\s/g, "");

assert(content.includes("\"development\";"), "support process.env.NODE_ENV");
assert(content.includes("\"aaa\""), "support String");
assert(content.includes("value:\"bbb\"") && content.includes("ccc:{"), "support Object");
assert(content.includes("[\"a\",1]"), "support Array");
assert(content.includes("console.log(1);"), "support Number");
assert(content.includes("console.log(true);"), "support Boolean");
assert(content.includes("console.log(false);"), "support Boolean");
assert(content.includes("console.log(null);"), "support Null");
assert(content.includes("console.log(2);"), "support expression");
