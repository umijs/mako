it("should interop dynamic_import_to_require", async () => {
  const lazy = await import('./lazy.ts')

  expect(lazy).toEqual({
    create: () => {}
  })
});

