const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const sharedChunks = Object.entries(files).filter(([fileName]) => /shared_\d+\.js$/.test(fileName));

const vendorChunks = Object.entries(files).filter(([fileName]) => /vendors_\d+\.js$/.test(fileName));

assert(
  sharedChunks.length === 5 && sharedChunks.every(([_, content]) => content.includes("some own module")),
  "should split all shared chunks"
);

assert(
  vendorChunks.length === 8 && vendorChunks.every(([_, content]) => content.includes("a module installed from npm")),
  "should split all vendors chunks"
);

