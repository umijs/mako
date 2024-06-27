import { value } from "./proxy";
import { setValue } from "./module";

it("should keep shared reference after concatenate", () => {
  expect(value).toBe(0);
  setValue(42);
  expect(value).toBe(42);
});
