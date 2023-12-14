const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

assert(content.includes(`rgba(0,0,0,0)`), "should have default exports in a.js");
assert(content.includes(`boxShadow`), "should have boxShadow exports in a.js");
assert(content.includes(`const b = 'b'`), "should have b exports in b.js");
assert(!content.includes(`const b1`), "should not have b1 exports in b.js");
assert(content.includes(`const a = 'a';`), "should have a exports in c.js");
assert(content.includes(`const a1 = 'a1';`), "should have a1 exports in c.js");
assert(!content.includes(`const a2`), "should not have a2 exports in c.js");
