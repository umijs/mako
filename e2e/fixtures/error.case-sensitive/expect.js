const assert = require("assert");

module.exports = (err) => {
  assert(
    err.stderr.includes(`/Assets/umi-logo.png does not match the corresponding path on disk [assets]`),
    "should throw error"
  );
};
