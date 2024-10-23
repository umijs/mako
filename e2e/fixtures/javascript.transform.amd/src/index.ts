// @ts-ignore
import a from './a'

it("amd/umd should be exports as commonjs", () => {
  expect(a).toEqual(1)
});
