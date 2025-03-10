const assert = require("assert");

module.exports = (err) => {
  assert(
    err.stderr.includes(`Unrecognised input`),
    "should throw error"
  );
};
