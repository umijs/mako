var testData = require("./src/index.js");

const nsObj = i=>i;


console.log(testData);

it("should export the correct values", function() {
	expect(testData).toStrictEqual(
		nsObj({
			svg5: nsObj({
				svg: nsObj({
					clinical1: {
						svg1: 1
					},
					clinical2: {
						svg2: 2
					}
				})
			}),
			svg6: nsObj({
				svg: nsObj({
					test: {
						svg1: 10
					},
					clinical2: {
						svg2: 20
					}
				})
			})
		})
	);
})
