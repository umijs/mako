const assert = require("assert");
const { parseBuildResult, string2RegExp } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files['index.js'].includes(`global["foo' oo"]`),
  "chunk loading should work in entry"
);

assert(
  files['src_a_ts-async.js'].includes(
    `(typeof globalThis !== "undefined" ? globalThis : self)["foo' oo"]`
  ),
  "chunk loading should work in async chunk"
);

