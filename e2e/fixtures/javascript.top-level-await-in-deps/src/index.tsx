import b from './b';
const cjs_b  = require("./b");

it("should got raw default export from async module",  () => {
    expect(b).toBe(1)
});

it("should return promise when use require directly", ()=>{
    expect(cjs_b).toBeInstanceOf(Promise);
})
