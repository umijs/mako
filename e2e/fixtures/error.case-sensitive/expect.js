const assert = require("assert");
const os = require("os");

module.exports = (err) => {
  if (os.platform() === "darwin") {
    assert(
      err.stderr.includes(
        `/Assets/umi-logo.png does not match the corresponding path on disk [assets]`
      ),
      "should throw error"
    );
  } else {
    assert(
      err.stderr.includes(
        `Module not found: Can't resolve './Assets/umi-logo.png'`
      ),
      "should throw error"
    );
  }
};
