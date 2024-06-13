const assert = require("assert");
const {
  parseBuildResult,
  moduleReg,
  injectSimpleJest,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

injectSimpleJest();

const index = files["index.js"];

expect(index).toContain(
  'var interop = __mako_require__("@swc/helpers/_/_interop_require_wildcard")._;',
);
expect(index).toContain('then(__mako_require__.dr(interop, "src/cjs.js"))');
