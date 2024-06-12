import { getCalled } from "./recorder";
require("./root");

it("keep the order in root", function () {
  expect(getCalled()).toStrictEqual(["inner", "ext"]);
});
