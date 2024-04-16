const { a, b } = require("./c");

it("should have the correct values", function() {
	expect(a()).toBe("a");
	expect(b()).toBe("b");
});
