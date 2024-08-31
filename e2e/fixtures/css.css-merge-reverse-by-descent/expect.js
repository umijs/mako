const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.css"];

assert(
  content.includes(`
.b {
  color: blue;
}
.c {
  color: green;
}
.a {
  color: red;
}
.d {
  color: black;
}`.trim()),
  "css merge in js should work"
);
