const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(!content.includes("node_modules/pure"), `should skip pure module`);
assert(content.includes("node_modules/side_effects/index.js"), `should keep all side effects modules`);
assert(content.includes("node_modules/side_effects/dep.js"),   `should keep all side effects modules`);

assert(content.includes("index.default"), `should change field name`);
