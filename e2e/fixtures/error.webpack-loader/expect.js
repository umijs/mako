const assert = require("assert");

module.exports = (err) => {
  assert(
    err.stderr.split("webpack loader syntax is not supported").length === 3,
    "should throw error when using webpack loader syntax"
  );
};
