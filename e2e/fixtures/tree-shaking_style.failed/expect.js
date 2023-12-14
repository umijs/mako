const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

assert(
  !content.includes(`"src/foo.tsx":`),
  "should not have src/foo.tsx module define"
);
