const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files["index.js"].includes(`window.require`),
  "should have window.require"
);

assert(
  files["index.js"].includes(`foo(b.require)`),
  "require replacement should work correctly"
);

assert(
  files["index.js"].includes(`require('foo')`),
  "require replacement should work correctly"
);

assert(
  files["index.js"].includes(`f.require`),
  "require replacement should work correctly"
);
