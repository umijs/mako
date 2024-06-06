const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files["src_workerHelper_ts-worker.js"].includes(
    'new Worker(new URL("src_workerHelper_ts-async.js",'
  ),
  "should have self-spawn codes"
);
