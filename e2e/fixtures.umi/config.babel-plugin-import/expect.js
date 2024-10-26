const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["p__index-async.js"];

assert.doesNotMatch(
  content,
  moduleReg('node_modules/antd/index.js', ''),
  'should not import antd entry',
);
assert.match(
  content,
  moduleReg('node_modules/antd/lib/button/index.js', "'Button'"),
  'should import antd button',
);

assert.doesNotMatch(
  content,
  moduleReg('node_modules/antd1/index.js', ''),
  'should not import antd1 entry',
);
assert.match(
  content,
  moduleReg('node_modules/antd1/es/button/index.js', "'Button1'"),
  'should import antd1 button',
);
assert.match(
  content,
  moduleReg('node_modules/antd1/es/button/style/index.js', "'style'"),
  'should import antd1 button style',
);
