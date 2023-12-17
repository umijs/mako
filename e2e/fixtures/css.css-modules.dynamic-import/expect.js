const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  trim(files["src_index_css-async.css"]).includes(`.container{`),
  "import('./index.css') should not be css modules"
);
