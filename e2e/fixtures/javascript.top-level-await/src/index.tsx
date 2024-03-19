async function af() {
    return 42
}

let ans= await af();

it("top level await should work", () => {
    expect(ans).toBe(42)
});
