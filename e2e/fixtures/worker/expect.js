const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
const content = files["index.js"];

assert(
  !Object.entries(files).every(([fileName, _]) => fileName.startsWith("src_worker_ts")),
  "should not generate worker file for worker.ts"
);
assert(
  content.includes(`const worker = new Worker('./worker.ts');`),
  "should not generate worker file for worker.ts"
);
assert(
  Object.entries(files).some(([fileName, _]) => fileName.startsWith("src_worker2_ts")),
  "should generate worker file for worker2.ts"
);
