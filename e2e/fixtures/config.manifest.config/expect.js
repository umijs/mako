const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert("manifest.json" in files, "should have manifest.json");

const manifest = JSON.parse(files["manifest.json"]);
assert("aaa/index.js" in manifest, "should have key: aaa/index.js");
assert("aaa/index.js.map" in manifest, "should have key: aaa/index.js.map");
