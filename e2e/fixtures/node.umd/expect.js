const path = require("path");
const assert = require("assert");

const file = path.join(__dirname, "dist/index.js");
const result = require(file).default();

assert(
  result === 'fooooo',
  "should run umd build result correctly"
);
