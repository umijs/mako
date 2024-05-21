import a from "./module_fn";

it("should have the correct values", function() {
	expect(a([1,2],[3,4])).toBe(10);
});
