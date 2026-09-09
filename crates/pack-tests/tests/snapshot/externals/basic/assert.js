const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const outputDir = path.join(__dirname, "output");
const runtimeFile = fs
  .readdirSync(outputDir)
  .find((file) => file.startsWith("turbopack-") && file.endsWith(".js"));
assert.ok(runtimeFile, "expected a browser entry runtime");
const runtime = fs.readFileSync(path.join(outputDir, runtimeFile), "utf8");

assert.match(
  runtime,
  /contextPrototype\.x = externalRequire/,
  "browser runtime must include CommonJS external support",
);
assert.match(
  runtime,
  /contextPrototype\.y = externalImport/,
  "browser runtime must include ESM external support",
);
