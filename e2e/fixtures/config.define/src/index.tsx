process.env.NODE_ENV;

declare var FOO: string;

it("defined string value should be right", () => {
  expect(AAA).toEqual("aaa")
});
it("defined number value should be right", () => {
  expect(BBB).toEqual(1)
});
it("defined boolean true value should be right", () => {
  expect(CCC).toEqual(true)
});
it("defined boolean false value should be right", () => {
  expect(DDD).toEqual(false)
});
it("defined null value should be right", () => {
  expect(EEE).toEqual(null)
});
it("defined array value should be right", () => {
  expect(FFF).toEqual(["a", 1])
});
it("defined caculcation value should be right", () => {
  expect(GGG).toEqual(2)
});
it("defined complex object value should be right", () => {
  expect(HHH).toEqual({
    "value": "bbb",
    "ccc": {
      "d": 1,
      "e": "2",
      "c": [
        1,
        "2",
        true
      ]
    }
  })
});
it("defined stringified object value should be right", () => {
  expect(III).toEqual({ v: 1 })
});


it("should replace ts declare shallowed value", ()=>{
  expect(FOO).toEqual("bar")
});
