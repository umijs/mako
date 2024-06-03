const assert = require('assert');

class T {
  a;
  b() {
    return this.a;
  }
}

let t = new T();
assert.deepStrictEqual(Object.getOwnPropertyDescriptor(t, 'a'), {
  value: undefined,
  writable: true,
  enumerable: true,
  configurable: true,
});
