const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const filePaths = Object.keys(files);
const hasCSS = filePaths.some((path) => path.endsWith(".css"));
assert(!hasCSS, "should not emit css");

let content = files["index.js"];
assert(content.includes(`"foo": \`foo-`), 'should contain css modules map');
