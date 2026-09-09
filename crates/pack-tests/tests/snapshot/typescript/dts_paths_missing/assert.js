const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const issuesPath = path.join(__dirname, "issues");
assert.ok(fs.existsSync(issuesPath), "a missing runtime package must report an issue");
const issues = fs.readdirSync(issuesPath)
  .map((file) => fs.readFileSync(path.join(issuesPath, file), "utf8"))
  .join("\n");
assert.match(issues, /Module not found/);
assert.match(issues, /missing-dts-runtime/);
