const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert.match(
  trim(files["index.css"]),
  /.container-.{8}{padding-top:80px;}/,
  "css content is not expected"
);

assert(
  trim(files["src_a_css-async.css"]).includes(`.a{`),
  "import('./index.css') should not be css modules"
);

assert(
  trim(files["index.css"]).includes(`.b{`),
  "require('./b.css') should not be css modules"
);

assert(
  trim(files["index.css"]).includes(`.c{`),
  "import './c.css' should not be css modules"
);

assert(
  trim(files["index.css"]).includes(`.e{`),
  "const e = require('./e.css') should not be css modules"
);

assert(
  files["index.js"].includes(`__mako_require__.ensure("src/f.css").then((f)=>f);`),
  "import('./f.css') should ensure chunk first"
);
