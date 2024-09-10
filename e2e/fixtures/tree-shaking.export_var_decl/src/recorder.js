let store = [];
global.record = function (n) {
  store.push(n);
  return n;
};

export function getStore() {
  return store;
}
