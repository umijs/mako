const {
  parseBuildResult,
  injectSimpleJest,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

injectSimpleJest();

const index = files["index.js"];

require("./dist/index")


expect(index).toContain(
  'var interop = __mako_require__("@swc/helpers/_/_interop_require_wildcard")._;',
);
expect(index).toContain('Promise.resolve(__mako_require__("lazy.ts")).then(interop)');
