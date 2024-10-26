import { getStore } from "./recorder.js";
import "./export_var";

it("should keep side effects in export var", () => {
  expect(getStore()).toStrictEqual([42]);
});
