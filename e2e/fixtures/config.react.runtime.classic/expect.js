const assert = require("assert");
const { parseBuildResult, string2RegExp } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(content.includes(`React.createElement("div", {`), `use classic runtime`);
