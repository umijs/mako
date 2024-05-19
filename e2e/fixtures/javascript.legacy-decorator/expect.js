const assert = require("assert");
const {
  parseBuildResult,
  moduleReg,
  injectSimpleJest,
} = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];
injectSimpleJest();

assert(
  content.includes(`Foo = _ts_decorate._([`),
  "legacy decorator should works",
);

require("./dist/index.js");
