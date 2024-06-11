let called = [];

export function record(value) {
  called.push(value);
}

export function getCalled() {
  return called;
}
