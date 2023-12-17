const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let content = files["umi.js"];
content = content.replace(/\s/g, "");

assert(moduleReg("antd1", "(typeof globalThis !== 'undefined' ? globalThis : self).antd1", true));
assert(moduleReg("antd2", "(typeof globalThis !== 'undefined' ? globalThis : self).antd2", true));
assert(moduleReg("antd3", "(typeof globalThis !== 'undefined' ? globalThis : self).antd3", true));
assert(moduleReg("antd4", "require.loadScript('https://example.com/lib/antd4.min.js'", true));
