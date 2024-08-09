const assert = require('assert');
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files);

assert(names.length === 4, "should emit chunk filename oversize of os limit 255 successfully")
