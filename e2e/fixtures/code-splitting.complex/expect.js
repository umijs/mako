const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert(
  !("should-be-merge_ts-async.js" in files),
  "minimal async chunk should be merged"
);

assert(
  files["index.js"].includes("src_isolated_ts-async.js"),
  "isolated dependences should connect to chunksIdToUrlMap"
)

assert(
  files["index.js"].includes("src_should-not-be-merged_ts-async.js"),
  "css module chunk can not be merged"
)

assert(
  files["index.js"].includes("src_no-incoming_tsx-async.js"),
  "should auto add edge to entry chunk after optimize remove chunk for next directed"
)

assert(
  Object.keys(files).every((f) => !f.includes("_isNumeric_js")),
  "empty chunk should be removed"
);

assert(
  "vendors_0-async.js" in files
      && "vendors_1-async.js" in files,
  "big vendors should be split again"
);

assert(
  files["index.js"].includes("\"src/context.ts\":")
    && !files["src_should-be-split_ts-async.js"].includes("\"src/context.ts\":"),
  "async chunk should reuse modules that already merged into entry with another minimal async chunk"
);

assert.match(
  files["index.js"].replace(/\s/g, ""),
  new RegExp(`Promise.all\\(\\[${
    [
      "common",
      "vendors_0",
      "vendors_1",
      "src/should-be-split.ts",
    ].map((f) => `__mako_require__.ensure\\("${f}"\\)`).join(",")
  }\\]\\)`),
  "should ensure splitting dependent chunks on demand (full)"
);

assert.match(
  files["index.js"].replace(/\s/g, ""),
  new RegExp(`Promise.all\\(\\[${
    [
      "common",
      "vendors_1",
      "src/other-dynamic.ts",
    ].map((f) => `__mako_require__.ensure\\("${f}"\\)`).join(",")
  }\\]\\)`),
  "should ensure splitting dependent chunks on demand (not-full)"
);

assert.doesNotMatch(
  files["common-async.js"],
  moduleReg("src/should-not-be-common.ts", "tooLongText"),
  "should not merge common async chunk to common chunk"
);

assert(
  "common-async.js" in files,
  "common async modules should be split"
);

assert(
  files["index.js"].includes('node_modules/antd/es/button/index.js": function') &&
  !files["vendors_0-async.js"].includes('node_modules/antd/es/button/index.js": function') &&
  !files["vendors_1-async.js"].includes('node_modules/antd/es/button/index.js": function'),
  "async chunks should reuse shared modules from entry chunk"
)
