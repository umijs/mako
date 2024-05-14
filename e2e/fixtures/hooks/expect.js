const assert = require("assert");

const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];
assert(content.includes(`(0, _jsxdevruntime.jsxDEV)(Foooo, {`), `jsx in foo.bar works`);
