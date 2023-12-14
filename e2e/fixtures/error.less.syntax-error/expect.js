const assert = require("assert");

module.exports = (err) => {
  assert(
    err.stderr.includes(`ParseError: Unrecognised input`),
    "should throw error"
  );
};
