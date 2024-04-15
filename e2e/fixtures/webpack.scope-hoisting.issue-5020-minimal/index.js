var testData = require("./src/index.js");
const nsObj = i=>i
it("should export the correct values", function() {
	expect(testData).toEqual(
		nsObj({
			icon: nsObj({
				svg: nsObj({
					default: 1
				})
			})
		})
	);
});
