const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const workers = Object.entries(files).filter(([fileName, content]) =>
  fileName.endsWith("src_workerHelpers_js-worker.js")
);

assert(workers.length === 1, "should generate one workerHelpers chunks");

assert(
  workers[0][1].includes(workers[0][0]),
  "should generate one workerHelpers chunks"
);
