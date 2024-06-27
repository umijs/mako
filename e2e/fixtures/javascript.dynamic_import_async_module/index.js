it("should have the correct values", async function () {
  let asyncModule = await import("./async");

  expect(asyncModule.named).toBe("named");
  expect(asyncModule.default).toBe("default");
});
