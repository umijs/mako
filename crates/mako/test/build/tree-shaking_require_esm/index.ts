function a() {}

function b(old: boolean = false) {
  if (old === true) {
    const { b } = require('./a');
    return b;
  }
  return a;
}

b();
