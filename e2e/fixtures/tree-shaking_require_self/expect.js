const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

const matches = content.match(/__mako_require__\(\"src\/index.ts\"\)/g);
assert(matches.length === 2, "require of src/index.ts should be twice");
