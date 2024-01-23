const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const filePaths = Object.keys(files);
const hasImage = filePaths.some((path) => path.endsWith(".jpg"));
assert(!hasImage, "should not emit image");
