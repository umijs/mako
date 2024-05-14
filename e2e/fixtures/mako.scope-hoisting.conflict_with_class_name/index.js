const m = require("./root.js")


it("should not conflict error",()=>{
	expect(m.default).toBeInstanceOf(Function)
});

