const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
const content = files["index.js"];

assert(
  !("src_worker_ts-worker.js" in files),
  "should not generate worker file for worker.ts"
);
assert(
  content.includes(`const worker = new Worker('./worker.ts');`),
  "should not generate worker file for worker.ts"
);
assert(
  "src_worker2_ts-worker.js" in files,
  "should generate worker file for worker2.ts"
);
