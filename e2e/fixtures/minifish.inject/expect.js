const assert = require('assert');
const { parseBuildResult, moduleReg } = require('../../../scripts/test-utils');
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(',');
const content = files['app.js'];

assert(
  content.includes(`var my = require("./mock-helper.js").minifishMockedMy;`),
  'inject wrong content',
);
