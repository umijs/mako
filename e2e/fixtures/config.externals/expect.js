const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert.match(
  content,
  moduleReg(
    "hoo",
    "module.exports = \\(typeof globalThis !== 'undefined' \\? globalThis : self\\).hoo;"
  ),
  "should have external module: hoo"
);

assert(
  content.includes(`module.exports = require("hoo");`),
  `should have external module: hoo_require`,
);

assert(
  content.includes(`module.exports = require("foo");`),
  `should have external module: foo_require`,
);

assert.match(
  content,
  moduleReg(
    "empty",
    "module.exports = '';"
  ),
  "should have external module: empty"
);

assert.match(
  content,
  moduleReg(
    "node_modules/antd/es/button/style/index.js",
    "console.log('style')",
    true
  ),
  "should not external style subpath"
);

assert.match(
  content,
  moduleReg("antd/es/locale/zh_CN", "module.exports = ''"),
  "should external subpath to empty"
);

assert.match(
  content,
  moduleReg(
    "antd/es/version",
    "module.exports = \\(typeof globalThis !== 'undefined' \\? globalThis : self\\).antd.version;"
  ),
  "should external 1-level subpath"
);

assert.match(
  content,
  moduleReg(
    "antd/es/date-picker",
    "module.exports = \\(typeof globalThis !== 'undefined' \\? globalThis : self\\).antd.DatePicker;"
  ),
  "should external 1-level subpath with PascalCase"
);

assert.match(
  content,
  moduleReg(
    "antd/es/input/Group",
    "module.exports = \\(typeof globalThis !== 'undefined' \\? globalThis : self\\).antd.Input.Group;"
  ),
  "should external 2-level subpath with PascalCase"
);

assert.match(
  content,
  moduleReg(
    "script",
    "__mako_require__.loadScript\\('https://example.com/lib/script.js'[^]+\\(typeof globalThis !== 'undefined' \\? globalThis : self\\).ScriptType"
  ),
  "should external in script mode"
);

assert.match(
  content,
  moduleReg(
    "src/index.tsx",
    `handleAsyncDeps\\(\\[\\s+_async__mako_imported_module_0__\\s+\\]\\)`,
  ),
  "should handle async script external"
);
