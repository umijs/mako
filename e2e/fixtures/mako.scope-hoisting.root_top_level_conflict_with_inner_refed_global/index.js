const { inner, default: root } = require("./root.js")


it("should keep root named unchanged",()=>{
	expect(root).toBe(42);
	expect(inner).toBe(undefined);
})
