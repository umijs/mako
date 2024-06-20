const assert = require("assert");

const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];
assert(content.includes(`children: "foo.bar"`), `jsx in foo.bar works`);
assert(content.includes(`children: ".bar"`), `jsx in hoo.bar works`);
assert(content.includes(`children: ".haha"`), `plugin in node_modules works`);
assert(content.includes(`children: ".hoo"`), `relative plugin works`);
