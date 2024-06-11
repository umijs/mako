const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  /__mako_require__\("src\/constants.ts"\)/,
  "should replace require to __mako_require__ for normal module"
);
assert.match(
  content,
  /require\("fs"\)/,
  "should keep require for standard module"
);
assert.match(
  content,
  /require\("node:fs"\)/,
  "should keep require for standard module node:"
);
assert.match(
  content,
  /require\("fs\/promises"\)/,
  "should keep require for standard module subpath"
);
assert.match(
  content,
  /readFileSync\("src\/index.ts"/,
  "should transform __filename"
);
assert.match(
  content,
  /console\.log\('dirname', "src"\);/,
  "should transform __dirname"
);
assert(content.includes(`require('crypto');`), `should keep require for crypto`);
