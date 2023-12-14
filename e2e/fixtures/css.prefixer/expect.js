const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.css"];

assert(content.includes(`display: -ms-flexbox;`), "ie 10 prefixer should works");
