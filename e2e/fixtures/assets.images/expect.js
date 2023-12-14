const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

// check files
assert.match(names, /umi-logo.(.*).png/, "should have umi-logo.png");
assert.match(
  names,
  /mailchimp-unsplash.(.*).jpg/,
  "should have mailchimp-unsplash.jpg"
);
assert.match(
  names,
  /bigfish-doctor-poster.(.*).jpeg/,
  "should have bigfish-doctor-poster.jpeg"
);
assert.match(
  names,
  /bigfish-doctor.(.*).gif/,
  "should have bigfish-doctor.gif"
);

// check content
assert.match(
  content,
  moduleReg(
    "src/assets/umi-logo.png",
    "module.exports = `\\${__mako_require__.publicPath}umi-logo.(.*).png`;"
  ),
  "umi-logo.png's content is not correct"
);
assert.match(
  content,
  moduleReg(
    "src/assets/mailchimp-unsplash.jpg",
    "module.exports = `\\${__mako_require__.publicPath}mailchimp-unsplash.(.*).jpg`;"
  ),
  "mailchimp-unsplash.jpg's content is not correct"
);
assert.match(
  content,
  moduleReg(
    "src/assets/bigfish-doctor-poster.jpeg",
    "module.exports = `\\${__mako_require__.publicPath}bigfish-doctor-poster.(.*).jpeg`;"
  ),
  "bigfish-doctor-poster.jpeg's content is not correct"
);
assert.match(
  content,
  moduleReg(
    "src/assets/bigfish-doctor.gif",
    "module.exports = `\\${__mako_require__.publicPath}bigfish-doctor.(.*).gif`;"
  ),
  "bigfish-doctor.gif's content is not correct"
);
