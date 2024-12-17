const {
  injectSimpleJest,
  parseBuildResult,
} = require("../../../scripts/test-utils");

injectSimpleJest();
const {files} = parseBuildResult(__dirname);
let filename = Object.keys(files)[0]

expect(filename).toMatch(/index\.umd\..+\.js/)
