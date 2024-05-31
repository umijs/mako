const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const entryDistContent = Object.entries(files).find(([k, v]) => /index\..{8}\.js/.test(k))[1];

assert(
  entryDistContent.includes('"src/index.ts":"index.ef3f45bc.css"'),
  "should correctly replace entry css chunk hash"
);

assert(
  entryDistContent.includes('"src/lazy.ts":"src_lazy_ts-async.d869db5c.js"'),
  "should correctly replace async js chunk hash"
);

assert(
  entryDistContent.includes('"src/lazy.ts":"src_lazy_ts-async.f7bbe864.css"'),
  "should correctly replace async css chunk hash"
);
