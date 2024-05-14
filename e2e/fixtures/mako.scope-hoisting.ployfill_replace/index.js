const { buffer } = require("./root.js")


it("should keep root named unchanged",()=>{
	expect(buffer).toBeInstanceOf(Buffer);
})
