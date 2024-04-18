const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
  !content.includes(`"src/foo.css"`),
  "should not contain css modules"
);

assert(
  !content.includes(`"src/hoo.module.css?module"`),
  "should not contain css-modules's css part module"
);

assert(
  content.includes(`"src/hoo.module.css?asmodule":`),
  "should contain css-module's js part module"
);

assert(
  content.includes(`Symbol.for("react.module.reference")`),
  "should transform client component to reference"
);

