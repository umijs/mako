// identifier
if ('production' !== process.env.NODE_ENV && process) {
  console.log(process.env.NODE_ENV, process);
} else {
  console.log('HAHA');
}

// empty module
const fs = require('fs');
console.log(fs);

// polyfill module
const path = require('path');
console.log(path.join('a', 'b'));
