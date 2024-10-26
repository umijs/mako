const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.css"];

assert(
  content.includes(`
.a {
  color: red;
}
.c {
  color: green;
}
.b {
  color: blue;
}
  `.trim()),
  "css merge in css multi imports should work"
);
