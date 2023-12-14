
try {
  require('./foo');
} catch(e) {}

try {
  exports.xx = require('./bar');
} catch(e) {}

try {
  const a = require('./hoo'), b = 1;
} catch(e) {}
