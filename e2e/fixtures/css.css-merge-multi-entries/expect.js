const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files['common.css'].includes(`
body::after {
  content: "common_1";
}
body::after {
  content: "common_2";
}
body::after {
  content: "common_3";
}
`.trim()),
  "css merge in mpa should works"
);
