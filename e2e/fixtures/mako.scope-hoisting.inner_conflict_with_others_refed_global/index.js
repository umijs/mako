
const { exported } = require("./root.js")


it("should keep root named unchanged",()=>{
	expect(exported).toStrictEqual({
		c: {log: 0xdead},
		inner: undefined,
	});
})
