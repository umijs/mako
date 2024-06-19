const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const workerDists = Object.entries(files).filter(([fileName, _]) => /^src_workerHelper_ts.*-worker.js$/.test(fileName));

assert(
  workerDists.length === 1 && workerDists[0][1].includes(
    `new Worker(new URL("${workerDists[0][0]}"`,
  ),
  "should have self-spawn codes"
);

assert(
  files["src_workerHelper_ts-async.js"].includes(
    `new Worker(new URL("${workerDists[0][0]}"`,
  ),
  "should instanitate worker with worker chunk"
);
