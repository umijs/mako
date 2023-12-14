const assert = require("assert");

module.exports = (err) => {
  assert(
    err.stderr.includes(`Module not found: Can't resolve 'foo'`),
    "should throw error when module not resolved"
  );
};
