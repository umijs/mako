import { no_exist, a1, b2, c3 } from "./module";
import no_exist_default from "./module";
export { not_exist_named } from "./module";

console.log(a1, b2, c3);

let x = { no_exist_default };

it("should keep shared reference after concatenate", () => {
  expect(no_exist).toBe(undefined);
  expect(no_exist_default).toBe(undefined);
  expect(x).toStrictEqual({ no_exist_default: undefined });
});
