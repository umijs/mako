const root = require("./root.js");
require("./ext");

it("skip conflict exports from inner", () => {
  expect(root).toStrictEqual({
    ext: "ext",
    value: "root",
    notConflict: "inner-not-conflict",
  });
});
