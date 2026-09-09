const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const output = fs
  .readdirSync("output")
  .filter((file) => file.endsWith(".js"))
  .map((file) => fs.readFileSync(path.join("output", file), "utf8"))
  .join("\n");

assert.match(output, /"react-compiler-runtime"/);
assert.doesNotMatch(output, /react\/compiler-runtime/);
assert.match(output, /const \$ = .*\["c"\]\)\(\d+\)/);
assert.match(output, /\$\[0\] !== count/);
