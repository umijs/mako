const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files["index.9a4a14f4.js"].includes('"src/index.ts":"index.ef3f45bc.css"'),
  "should correctly replace entry css chunk hash"
);

assert(
  files["index.9a4a14f4.js"].includes('"src/lazy.ts":"src_lazy_ts-async.0c40bab1.js"'),
  "should correctly replace async js chunk hash"
);

assert(
  files["index.9a4a14f4.js"].includes('"src/lazy.ts":"src_lazy_ts-async.f7bbe864.css"'),
  "should correctly replace async css chunk hash"
);
