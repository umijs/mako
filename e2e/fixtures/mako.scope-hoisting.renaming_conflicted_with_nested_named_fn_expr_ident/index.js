const root = require("./module");

it("should detect nested named fn expr ident", () => {
  let target = root.c.addTarget();

  expect(target).toBe("OK");
});
