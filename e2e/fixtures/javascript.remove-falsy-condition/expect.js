const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

assert(
  !content.includes(`console.log("false")`),
  "should remove if (false) condition"
);

assert(
  !content.includes(`console.log("false && true")`),
  "should remove if (false && true) condition"
);

assert(!content.includes(`console.log("0")`), "should remove if (0) condition");

assert(
  content.includes(`typeof process !==`),
  `should keep if (typeof process !== 'undefined') condition`
);

// TODO
// assert(
//   !content.includes(`test require`),
//   "should remove if (typeof require === 'undefined') condition"
// );

// TODO
// assert(
//   !content.includes(`test exports`),
//   "should remove if (typeof exports === 'undefined') condition"
// );
