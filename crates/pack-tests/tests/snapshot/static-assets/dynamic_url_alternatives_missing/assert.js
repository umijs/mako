const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const issues = fs
  .readdirSync(path.join(__dirname, "issues"))
  .map((name) => fs.readFileSync(path.join(__dirname, "issues", name), "utf8"))
  .join("\n");
assert.match(issues, /Module not found/);
assert.match(issues, /missing-static-asset\.txt/);
