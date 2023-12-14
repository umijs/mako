const assert = require("assert");

const { parseBuildResult, trim } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

assert.match(trim(files['index.css']), /@import"\/\/a\.com\/a\.css";/, "support // prefix url");
assert.match(trim(files['index.css']), /@import"http:\/\/b\.com\/b\.css";/, "support http:// prefix url");
assert.match(trim(files['index.css']), /@import"https:\/\/c\.com\/c\.css";/, "support https:// prefix url");
assert.match(trim(files['index.css']), /@import"HTTPS:\/\/d\.com\/d\.css";/, "support uppercase");
