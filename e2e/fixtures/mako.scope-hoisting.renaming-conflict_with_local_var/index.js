it("should check existing variables when renaming", function () {
  expect(require("./module").c.a()).toBe("ok-root");
  expect(require("./module").c.b()).toBe("ok");
});
