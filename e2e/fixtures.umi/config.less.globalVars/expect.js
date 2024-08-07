const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

/** @type string */
let content = files["pages_index_tsx-async.css"];

// remove sourcemap
content = content.slice(0, content.indexOf("/*")).trim();

// 分割成单条 rule
// .a{...}
// .b{...}
let cssRules = content
  .split(".")
  .filter((item) => !!item)
  .map((item) => "." + item.replace(/\s/g, ""));

/** @type {Record<string, string>} */
let cssDecls = cssRules.reduce((pre, cur) => {
  let splitIndex = cur.indexOf("{");
  let selector = cur.slice(0, splitIndex);
  let decl = cur.slice(splitIndex);
  pre[selector] = decl;

  return pre;
}, {});

assert(
  cssDecls[".normal"].includes("color:#ff0000;"),
  "should available globalVars"
);
assert(
  cssDecls[".override"].includes("color:#ff0022;"),
  "should available override globalVars"
);
assert(
  cssDecls[".inConfig"].includes("color:#ff0033;"),
  "should available globalVars in .umirc config"
);
assert(
  cssDecls[".overrideInConfig"].includes("color:#000012"),
  "should available override globalVars in .umirc config"
);
assert(
  cssDecls[".useAtPrefix"].includes("color:#000013"),
  "should available use '@' prefix"
);
assert(
  cssDecls[".normalA"].includes("color:#ff0000;"),
  "should available globalVars in all less files"
);
assert(
  cssDecls[".normalB"].includes("color:#ff0000;"),
  "should available globalVars in all less files"
);
