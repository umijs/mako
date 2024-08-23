const assert = require("assert");
const { parseBuildResult, string2RegExp } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.css"];

assert.doesNotMatch(
  content,
  /width: 0.34rem;/,
  "should not have `width: 0.34rem;`"
)

assert.match(
  content,
  /margin: 0 0 0.2rem;/,
  "should have `margin: 0 0 0.2rem;`"
);
assert.doesNotMatch(
  content,
  /content: 0.16rem;/,
  "should not have `content: 0.16rem;`"
);
assert(
  content.includes("@media (min-width: 5rem) {"),
  "media query should be transformed"
);
assert(
  content.includes(
    "--border-radius: var(--adm-button-border-radius, 0.88rem);"
  ),
  "should convert rpx in var func"
);
assert(
  content.includes(
    "height: var(--state, var(--button-color, var(--brand-color, 0.88rem)));"
  ),
  "should convert rpx in nested var func "
);
assert(
  content.includes("width: 0.88rem;"),
  "should convert when surrounding by var func declaration"
);
assert(
  content.includes(
    "--my-height: var(--state, var(--button-color, var(--brand-color, calc(100% - 0.88rem))));"
  ),
  "should convert rpx in nested var func and other func call"
);
assert(
  content.includes("--border-top: var(--adm-button-border-radius, 88em);"),
  "em should not be converted in var func"
);
assert(
  content.includes(
    "min-height: var(--state, var(--button-color, var(--brand-color, 88em)));"
  ),
  "em should not be converted in nested var func"
);
assert(
  content.includes(
    "--your-height: var(--state, var(--button-color, var(--brand-color, calc(100% - 88em))));"
  ),
  "em should not be converted in nested var and other func"
);
