const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert("chunk_a-async.js" in files
  && files["chunk_a-async.js"].includes('console.log("lazy_a_0")')
  && files["chunk_a-async.js"].includes('console.log("lazy_a_1")'),
"should have chunk_a-async.js");

assert("chunk_b-async.js" in files
  && files["chunk_b-async.js"].includes('console.log("lazy_b")'),
"should have chunk_b-async.js");

assert("my_worker-worker.js" in files
  && files["my_worker-worker.js"].includes('console.log("worker")'),
"should have my_worker-worker.js");
