const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const sharedChunks = Object.entries(files).filter(([fileName]) => /shared_\d+\.js$/.test(fileName));

const libChunks = Object.entries(files).filter(([fileName]) => /lib\d+\.js$/.test(fileName));

assert(
  libChunks.length === 2
  && libChunks.some(([fileName, content]) => fileName.includes("lib_lib1") && content.includes("node_modules/lib1/index.js"))
  && libChunks.some(([fileName, content]) => fileName.includes("lib_lib2") && content.includes("node_modules/lib2/index.js")),
  "should split all lib chunks"
);

assert(
  sharedChunks.length === 2
  && sharedChunks.some(([_, content]) => content.includes("node_modules/shared1/index.js"))
  && sharedChunks.some(([_, content]) => content.includes("node_modules/shared2/index.js")),
  "should split all shared chunks"
);

assert(
  libChunks.length === 1 && libChunks[0][1].includes("node_modules/framework1/index.js") && libChunks[0][1].includes("node_modules/framework1/index.js"),
  "should split framework chunks"
);

