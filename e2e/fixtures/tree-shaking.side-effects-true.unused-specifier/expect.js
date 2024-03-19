const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const content = files["index.js"];

assert(
    content.includes("/a.js") && 
        !content.includes("_a"),
    `should remove default specifier`
);

assert(
    content.includes("/b.js") && 
        !content.includes("_b"),
    `should remove namespace specifier`
);

assert(
    content.includes("/c.js") && 
        !content.includes("_c"),
    `should remove namespace specifier`
);