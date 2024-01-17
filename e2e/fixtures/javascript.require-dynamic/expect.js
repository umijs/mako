const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files);
const content = files["index.js"];
const asyncContent = files[names.find((name) => name.startsWith("src_i18n_"))];

assert.match(
  content,
  moduleReg(
    "src/i18n\\?context&glob=\\*\\*/\\*",
    [
      "'./zh-CN.json': ()=>__mako_require__(\"src/i18n/zh-CN.json\")",
      "'./zh-CN': ()=>__mako_require__(\"src/i18n/zh-CN.json\")",
    ].join(',\n\\s+'),
    true,
  ),
  "should generate context module with correct map",
);

assert.match(
  content,
  moduleReg(
    "src/fake.js\\?context&glob=\\*\\*/\\*",
    [
      "'./a': ()=>__mako_require__(\"src/fake.js/a.js\")",
      "'./index.js': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'./index': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'.': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'./': ()=>__mako_require__(\"src/fake.js/index.js\")",
    ].join(',\n\\s+'),
    true,
  ),
  "should generate context module for fake ext directory with correct map",
);

assert.match(
  asyncContent,
  moduleReg("src/i18n\\?context&glob=\\*\\*/\\*.json", "'./zh-CN.json': ()=>__mako_require__(\"src/i18n/zh-CN.json\")", true),
  "should generate context module with correct map in async chunk",
);

assert.match(
  content,
  moduleReg("src/index.ts", '__mako_require__.ensure("src/i18n\\?context&glob=\\*\\*/\\*.json")', true),
  "should generate async require for import dynamic module",
);

assert.match(
  content,
  moduleReg("src/index.ts", "`./\\${lang}.json`", true),
  "should replace string template prefix ./i18n/ with ./",
);

assert.match(
  content,
  moduleReg("src/index.ts", '__mako_require__("src/i18n\\?context&glob=\\*\\*/\\*")', true),
  "should generate sync require for require dynamic module",
);

assert.match(
  content,
  moduleReg("src/index.ts", '"." \\+ file', true),
  "should replace bin left string @/i18n with .",
);

assert.doesNotMatch(
  content,
  // /*.../glob=**/**/ should be escaped to /*.../glob=**\/**/
  //                                                     ^^
  /glob=\*\*\/\*\s*\*\//,
  "should escape glob pattern in module id debug comment"
);
