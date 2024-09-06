const assert = require("assert");

require('./dist/common');
assert(require('./dist/A').common === require('./dist/B').common, 'global module registry should work');
