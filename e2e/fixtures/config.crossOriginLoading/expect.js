const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);
const content = files["index.js"];

assert(content.includes("script.crossOrigin = 'anonymous'"), 'should set crossOrigin to anonymous for loadScript function');
assert(content.includes("link.crossOrigin = 'anonymous'"), 'should set crossOrigin to anonymous for createStylesheet function');
