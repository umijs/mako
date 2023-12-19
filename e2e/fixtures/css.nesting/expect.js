const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.css"];

assert(content.includes(`.foo .bar {`), "css nesting should works");
assert(content.includes(`.hoo {`), "css nesting with :global should works");
