const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files["framework.js"].includes('console.log("framework1")')
  && files["framework.js"].includes('console.log("framework2")')
  && !files["framework.js"].includes("normal"),
  "should split framework chunks"
);

assert(
  files["lib_0_lib1-async.js"].includes('console.log("lib1")')
  && !files["lib_0_lib1-async.js"].includes("normal")
  && files["lib_1_lib2-async.js"].includes('console.log("lib2")')
  && !files["lib_1_lib2-async.js"].includes("normal")
  && files["lib_0_shared1-async.js"].includes('console.log("shared1")')
  && !files["lib_0_shared1-async.js"].includes("normal")
  && files["lib_0_shared2-async.js"].includes('console.log("shared2")')
  && !files["lib_0_shared2-async.js"].includes("normal"),
  "should split lib chunks"
);

assert(
  files["shared_yDNCfB0E-async.js"].includes('console.log("s1_shared")')
  && !files["shared_yDNCfB0E-async.js"].includes("normal")
  && files["shared_QBWMs6xD-async.js"].includes('console.log("s2_shared")')
  && !files["shared_QBWMs6xD-async.js"].includes("normal"),
  "should split shared chunks"
);



