const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert("common.js" in files, "should have shared entry chunk");

assert.match(
  files["common.js"],
  moduleReg("src/utils.ts", "console.log('utils')", true),
  "should have common module in shared entry chunk"
);

assert.doesNotMatch(
  files["a.js"],
  moduleReg("src/utils.ts", "console.log('utils')", true),
  "should not have common module in entry chunk"
);

assert(
  files["a.js"].includes('"src/c.ts": "src_c_ts-async.js"'),
  "should have async chunk which belongs to entry chunk"
);

assert(
  !files["b.js"].includes('"src/c.ts": "src_c_ts-async.js"'),
  "should not have async chunk which not belongs to entry chunk"
);
