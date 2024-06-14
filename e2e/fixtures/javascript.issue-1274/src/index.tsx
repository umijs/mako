function forOf(y) {
  let r = [];
  for (let x of y) {
    r.push(x + 1);
  }
  return r;
}

it("for-of should work", () => {
  expect(forOf([1])).toStrictEqual([2]);
});

class Symbol {
  name() {
    return "AnotherSymbol";
  }
}

it("should overwrite Symbol", () => {
  let s = new Symbol();
  expect(s.name()).toBe("AnotherSymbol");
});
