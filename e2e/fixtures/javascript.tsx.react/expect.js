const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);


const content = files["index.js"];

assert.match(
  content,
  moduleReg("index.tsx", "__mako_require__(\"../../../node_modules/.pnpm/react@18.2.0/node_modules/react/index.js\")", true),
  "should keep require react programa",
  true
);


