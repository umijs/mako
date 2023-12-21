const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

assert(!content.includes("a-side-effects-false/index.js"), "should not include a-side-effects-false/index.js (barrel file)");
assert(content.includes(`var _a1export = _interop_require_wildcard._(__mako_require__("node_modules/a-side-effects-false/a1-export.js"));`), "should include a1-export.js");
assert(content.includes(`var _a2import = __mako_require__("node_modules/a-side-effects-false/a2-import.js");`), "should include a1-import.js");
assert(content.includes(`var _a3exportfrom = __mako_require__("node_modules/a-side-effects-false/a3-export-from.js");`), "should include a3-export-from.js");
assert(content.includes(`var _a4 = __mako_require__("node_modules/a-side-effects-false/a4.js");`), "should include a4.js");
assert(content.includes(`_a4.a41`), "should include _a4.a41");

assert(content.includes(`var _bsideeffectstrue = __mako_require__("node_modules/b-side-effects-true/index.js");`), "should include a-side-effects-true/index.js (barrel file) but sideEffects: true");

// TODO:
// [x] 1\ export * as foo from './foo';
// 2\ alias
// 3\ externals
// 4\ sideEffects: false + cjs
// 5\ cjs remix esm

/**
 * Expect:
 * a 和 c 的 import 都应该被替换成子路径
 * b 的 import 都不应该被替换成子路径
 */
