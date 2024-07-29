const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let content = files["pages_index_tsx-async.css"];
content = content.replace(/\s/g, "");
assert(content.includes(`background:#79caf2;`), "sassLoader should support");
