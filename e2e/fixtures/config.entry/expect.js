const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["foo.js"];

assert(content, `should have foo.js`);
assert(content.includes(`"src/bar.ts":`), `should have src/bar.ts module define`);
assert(content.includes(`"src/foo.ts":`), `should have src/foo.ts module define`);
assert(names.includes('hoo/hoo.js'), `should have hoo/hoo.js`);
assert(names.includes('soo.js'), `should have soo.js`);
assert(names.includes('yoo.js'), `should have yoo.js`);
