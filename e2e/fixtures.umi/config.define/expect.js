const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

let content = files["umi.js"];
content = content.replace(/\s/g, "");

assert(content.includes("\"production\""), "support process.env.NODE_ENV");
assert(content.includes("\"aaa\""), "support String");
assert(content.includes("value:\"bbb\"") && content.includes("ccc:{"), "support Object");
assert(content.includes("[\"a\",1]"), "support Array");
assert(content.includes("console.log(1);"), "support Number");
assert(content.includes("console.log(true);"), "support Boolean");
assert(content.includes("console.log(false);"), "support Boolean");
assert(content.includes("console.log(null);"), "support Null");
assert(content.includes("console.log('process.env',false);"), "support normalize process.env.XXX");

/**
"production";
"aaa";
({
    ccc: {
        e: "2",
        d: 1,
        c: [
            1,
            "2",
            true
        ]
    },
    value: "bbb"
});
1;
true;
false;
null;
[
    "a",
    1
];
 */
