const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files);
const content = files["index.js"];
const asyncContent = names.filter((name) => name.startsWith("src_i18n_")).reduce((acc, name) => acc + files[name], "");

assert.match(
  content,
  moduleReg(
    "src\\?context&glob=\\*\\*/\\*",
    [
      "'./ext/json.ts': ()=>__mako_require__(\"src/ext/json.ts\")",
      "'./ext/json': ()=>__mako_require__(\"src/ext/json.ts\")",
      "'./fake.js': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'./fake': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'./fake.js/a.js': ()=>__mako_require__(\"src/fake.js/a.js\")",
      "'./fake.js/a': ()=>__mako_require__(\"src/fake.js/a.js\")",
      "'./fake.js/aa.js': ()=>__mako_require__(\"src/fake.js/aa.js\")",
      "'./fake.js/aa': ()=>__mako_require__(\"src/fake.js/aa.js\")",
      "'./fake.js/index.js': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'./fake.js/index': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'./fake.js': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'./fake.js/': ()=>__mako_require__(\"src/fake.js/index.js\")",
      "'./i18n/en-US.json': ()=>__mako_require__(\"src/i18n/en-US.json\")",
      "'./i18n/en-US': ()=>__mako_require__(\"src/i18n/en-US.json\")",
      "'./i18n/zh-CN.json': ()=>__mako_require__(\"src/i18n/zh-CN.json\")",
      "'./i18n/zh-CN': ()=>__mako_require__(\"src/i18n/zh-CN.json\")",
      "'./index.ts': ()=>__mako_require__(\"src/index.ts\")",
      "'./index': ()=>__mako_require__(\"src/index.ts\")",
      "'.': ()=>__mako_require__(\"src/index.ts\")",
      "'./': ()=>__mako_require__(\"src/index.ts\")"
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
      "'./a.js': ()=>__mako_require__(\"src/fake.js/a.js\")",
      "'./a': ()=>__mako_require__(\"src/fake.js/a.js\")",
      "'./aa.js': ()=>__mako_require__(\"src/fake.js/aa.js\")",
      "'./aa': ()=>__mako_require__(\"src/fake.js/aa.js\")",
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
  moduleReg("src/i18n\\?context&glob=\\*\\*/\\*.json&async", "'./zh-CN.json': ()=>Promise.all([\n.*__mako_require__.ensure(\"src/i18n/zh-CN.json\")\n.*]).then(__mako_require__.bind(__mako_require__, \"src/i18n/zh-CN.json\"))", true),
  "should generate context module with correct map in async chunk",
);

assert.match(
  asyncContent,
  moduleReg("src/i18n/zh-CN.json", "中文", true),
  "should generate context module with correct map in async chunk",
);

assert.match(
  asyncContent,
  moduleReg("src/i18n\\?context&glob=\\*\\*/\\*.json&async", "'./en-US.json': ()=>Promise.all([\n.*__mako_require__.ensure(\"src/i18n/en-US.json\")\n.*]).then(__mako_require__.bind(__mako_require__, \"src/i18n/en-US.json\"))", true),
  "should generate context module with correct map in async chunk",
);

assert.match(
  asyncContent,
  moduleReg("src/i18n/en-US.json", "English", true),
  "should generate context module with correct map in async chunk",
);

assert.match(
  content,
  moduleReg("src/index.ts", '__mako_require__.ensure("src/i18n\\?context&glob=\\*\\*/\\*.json&async")', true),
  "should generate async require for import dynamic module",
);

assert.match(
  content,
  moduleReg("src/index.ts", 'ensure("src/i18n\\?context&glob=\\*\\*/\\*&async")', true),
  "should generate async require for import dynamic module with then callback",
);

assert.match(
  content,
  moduleReg("src/index.ts", "`./\\${lang}.json`", true),
  "should replace string template prefix ./i18n/ with ./",
);

assert.match(
  content,
  moduleReg("src/index.ts", "`./zh-\\${lang}.json`", true),
  "prefix should match the last one '/' ",
);


assert.match(
  content,
  moduleReg("src/index.ts", '__mako_require__("src\\?context&glob=\\*\\*/\\*")', true),
  "should generate sync require for require dynamic module",
);

assert.match(
  content,
  moduleReg("src/index.ts", '__mako_require__("src/ext\\?context&glob=\\*\\*/\\*")', true),
  "should generate nested sync require in dynamic require/import args",
);


assert.doesNotMatch(
  content,
  // /*.../glob=**/**/ should be escaped to /*.../glob=**\/**/
  //                                                     ^^
  /glob=\*\*\/\*\s*\*\//,
  "should escape glob pattern in module id debug comment"
);
