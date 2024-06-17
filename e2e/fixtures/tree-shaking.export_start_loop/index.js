import { a } from "./l1.js";

it("should import from export * looped module", () => {
  expect(a).toBe(1);
});
