import { bar, FooType } from "./type"

it("cant import value",()=>{
  expect(bar).toBe('bar')
});

it('export namespace type as undefined',()=>{
  expect(FooType).toBe(undefined)
})
