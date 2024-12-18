const assert = require("assert");

assert(require("./dist/index.js") === "foo", "umd export should work")
