const assert = require("assert");

module.exports = (err) => {
  assert(
    err.stderr.includes(`'foo' wasn't found.`),
    "should throw error"
  );
};
