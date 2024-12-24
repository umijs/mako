const assert = require("assert");

const { parseBuildResult } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const content = files['index.js'];
assert(content.includes('fooooooo'), `should replace FOOOO with "fooooooo" with unplugin-replace`);
assert(content.includes('fill: "currentColor",'), `should include fill: "currentColor" with unplugin-icons`);
