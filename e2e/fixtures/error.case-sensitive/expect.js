const assert = require("assert");
const os = require("os");
const { files } = parseBuildResult(__dirname);

module.exports = (err) => {
  if (os.platform() === 'darwin') {
    assert(
      err.stderr.includes(`/Assets/umi-logo.png does not match the corresponding path on disk [assets]`),
      "should throw error"
    );
  } else {
    assert("index.js" in files, "should have file: index.js");
  }
};
