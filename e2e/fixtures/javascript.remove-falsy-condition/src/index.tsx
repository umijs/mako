if (false) {
  require("not-found");
  console.log("false");
}

if (false && true) {
  console.log("false && true");
}

// should keep
if (typeof process !== "undefined") {
  console.log('typeof process !== "undefined"');
}

if (0) {
  console.log("0");
}

// TODO
if (typeof require === 'undefined') {
  console.log("test require");
  require('fs').readFileSync('abc', 'utf8');
}

// TODO
if (typeof exports === 'undefined') {
  console.log("test exports");
}
