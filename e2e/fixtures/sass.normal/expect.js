const assert = require("assert");
const { parseBuildResult,trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  files["index.css"].includes(
    `background: #79caf2;`
  ),
  "should support variables"
);

assert(
  files["index.css"].includes(
    `nav ul {`
  ),
  "should support nesting"
);

assert(
  files["index.css"].includes(
    `color: #333;`
  ),
  "should support modules"
);

assert(
  trim(files["index.css"]).includes(
    trim(`.info {
  background: #a9a9a9;
  box-shadow: 0 0 1px rgba(169, 169, 169, .25);
  color: #fff;
}`)
  ),
  "should support mixins"
);

assert(
  trim(files["index.css"]).includes(
    trim(`article[role=main] {
  width: 62.5%;
}`)
  ),
  "should support operators"
);
