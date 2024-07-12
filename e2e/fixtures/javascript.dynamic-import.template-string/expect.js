const {
  parseBuildResult,
} = require("../../../scripts/test-utils");
const assert = require("assert");
const { files } = parseBuildResult(__dirname);
const content = files["index.js"];

assert(content.includes(`"foo.js": "foo_js-async.js"`), "template string replace works");
