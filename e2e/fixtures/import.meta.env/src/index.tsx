it("should replace import.meta.env", () => {
  expect(import.meta.env).toEqual({MODE: "development"})
  expect(import.meta.env.MODE).toEqual("development")
});
