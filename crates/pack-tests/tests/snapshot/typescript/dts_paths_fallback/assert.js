const assert = require("node:assert/strict");

assert.equal(require("./output/main.js").default, "dts-exact-runtime");
