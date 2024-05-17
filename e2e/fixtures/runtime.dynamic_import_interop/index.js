it("should interop cjs module with default", async () => {
  let cjs = await import("./src/cjs");

  expect(cjs).toEqual({ default: { foo: 42 }, foo: 42 });
});
