const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

// js 会保留 side-effects 为 false 的模块
assert(
  !content.includes(`"node_modules/js-side-effects-false/index.js":`),
  `should not have js-side-effects-false module define`,
);
// TODO: FIXME
assert(
  content.includes(`"node_modules/js-side-effects-true/index.js":`),
  `should have js-side-effects-true module define`,
);

// ts 不管 side-effects 全删
assert(
  !content.includes(`"node_modules/ts-side-effects-false/index.js":`),
  `should not have ts-side-effects-false module define`,
);
assert(
  !content.includes(`"node_modules/ts-side-effects-true/index.js":`),
  `should not have ts-side-effects-true module define`,
);
