const assert = require("assert");
const { parseBuildResult, trim, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert("asset-manifest.json" in files, "should have asset-manifest.json");

const manifest = JSON.parse(files["asset-manifest.json"]);
assert("index.js" in manifest, "should have key: index.js");
assert("index.js.map" in manifest, "should have key: index.js.map");
