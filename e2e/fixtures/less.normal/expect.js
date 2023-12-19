const assert = require("assert");
const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert.match(
  files["index.css"],
  /.container-.{8} {/,
  "should have hash in name"
);
assert.match(
  files["index.css"],
  /padding-top: 80px/,
  "should have correct style"
);
assert(
  files["index.css"].includes(
    `grid-template: repeat(1, 1fr) / repeat(3, 1fr);`
  ),
  "should not panic when parsing grid-template"
);
