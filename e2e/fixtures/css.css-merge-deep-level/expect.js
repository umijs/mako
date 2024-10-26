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
.b {
  color: blue;
}
.c {
  color: green;
}
.b1 {
  color: blue;
}
.c1 {
  color: green;
}
.a1 {
  color: red;
}
  `.trim()),
  "css merge deep level should work"
);
