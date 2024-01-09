const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");

const { files } = parseBuildResult(__dirname);
const content = files["index.js"];

assert(content.includes(`/* foo */`), "comments should be kept");
