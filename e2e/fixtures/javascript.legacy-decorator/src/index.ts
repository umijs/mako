@bar()
class Foo {}

function bar() {
  return function (theClass) {
    theClass.bar = true;
  };
}

it("should run the decortate", () => {
  expect(Foo.bar).toBe(true);
});
