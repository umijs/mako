const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const output = path.join(__dirname, "output");
const files = fs.readdirSync(output);
const scripts = files.filter((file) => /^main\.[a-f0-9]{8}\.js$/.test(file));
assert.equal(scripts.length, 1);
const script = scripts[0];
const content = fs.readFileSync(path.join(output, script), "utf8");
const comments = [...content.matchAll(/[#@]\s*sourceMappingURL=([^\s*]+)/g)];
assert.equal(comments.length, 1);
assert.equal(decodeURIComponent(comments[0][1]), `${script}.map`);
const map = JSON.parse(
  fs.readFileSync(path.join(output, `${script}.map`), "utf8"),
);
assert.equal(map.version, 3);
assert.deepEqual(
  files.filter((file) => file.endsWith(".map")),
  [`${script}.map`],
);

const license = `${script}.LICENSE.txt`;
assert.ok(content.includes(license));
assert.match(
  fs.readFileSync(path.join(output, license), "utf8"),
  /package license/,
);

const sandbox = { URL };
vm.runInNewContext(content, sandbox, { timeout: 2000 });
assert.equal(sandbox.MyLibrary.value, 42);
