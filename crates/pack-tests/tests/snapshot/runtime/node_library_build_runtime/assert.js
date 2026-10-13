const assert = require("node:assert/strict");
const path = require("node:path");
const { pathToFileURL } = require("node:url");

const library = require("./output/main.js");
const entryUrl = pathToFileURL(path.join(__dirname, "input/index.js")).href;
const dependencyUrl = pathToFileURL(
  path.join(__dirname, "input/meta space % 中文.mjs"),
).href;
const expectedEnv = {
  DEV: false,
  PROD: true,
  MODE: "production",
  BASE_URL: "/",
  SSR: true,
};

assert.equal(library.answer(), 42);
assert.equal(library.entryUrl, entryUrl);
assert.equal(library.readEntryUrl(), entryUrl);
assert.equal(library.dependencyUrl, dependencyUrl);
assert.equal(library.readDependencyUrl(), dependencyUrl);
assert.deepEqual({ ...library.entryEnv }, expectedEnv);
assert.deepEqual({ ...library.dependencyEnv }, expectedEnv);
