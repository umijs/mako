const root = require("./root")

it("should export all exports from inner", function() {
	expect(root).toStrictEqual({a:1,b:2,c:3});
});
