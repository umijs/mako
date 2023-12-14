const assert = require("assert");
const { parseBuildResult, string2RegExp } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  string2RegExp(
    "if (typeof exports === 'object' && typeof module === 'object') module.exports = factory();"
  ),
  "should have exports=foooo"
);
assert.match(
  content,
  string2RegExp(
    "else if (typeof define === 'function' && define.amd) define([], factory);"
  ),
  "should have define foooo"
);
assert.match(
  content,
  string2RegExp(
    "else if (typeof exports === 'object') exports['foooo'] = factory();"
  ),
  "should have exports['foooo']"
);
assert.match(
  content,
  string2RegExp("else root['foooo'] = factory();"),
  "should have root['foooo']"
);
