const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = trim(files["index.css"]);

assert.match(
  content,
  /-webkit-transition:/,
  "missing -webkit prefix on: transition"
);
assert.match(
  content,
  /-webkit-user-select:/,
  "missing -webkit prefix on: user-select"
);
assert.match(
  content,
  /background:-webkit-linear-gradient\(/,
  "missing -webkit prefix on: linear-gradient"
);
