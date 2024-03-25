it("should keep export name", () => {
	return import("./module").then(mod => {
		expect(mod.__esModule).toBe(true);
		expect(mod.conflict).toBe(2333);
	})
})
