
const { named } = require("./root.js")


it("should keep root named unchanged",()=>{
	expect(named).toBe(42);
})
