const assert = require("assert");

module.exports = (err) => {
  assert(
    err.stderr.includes(`Entry is empty`),
    "should throw error"
  );
};
