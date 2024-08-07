const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(content.includes(`__mako_require__.publicPath = '/foo/'`), `__webpack_public_path__ works`);
assert(content.includes(`__mako_require__.publicPath = '/bar/'`), `__mako_public_path__ works`);
