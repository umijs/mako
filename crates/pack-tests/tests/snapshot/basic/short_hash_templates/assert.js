const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const outputDir = path.join(__dirname, "output");
assert(fs.existsSync(path.join(outputDir, "scripts/main.js")));

for (const [directory, extension] of [
  ["chunks", "js"],
  ["styles", "css"],
]) {
  const files = fs.readdirSync(path.join(outputDir, directory));
  assert(files.some((file) => file.endsWith(`.${extension}`)));
  for (const file of files) {
    assert.match(
      file,
      new RegExp(`^.+\\.[0-9a-f]{8}\\.${extension}(?:\\.map)?$`),
    );
  }
}

assert(
  fs
    .readdirSync(path.join(outputDir, "chunks"))
    .some((file) =>
      file.includes(
        "directory_name_that_must_not_appear_in_production_chunk_urls_lazy",
      ),
    ),
  "[name] templates must preserve readable chunk names",
);
