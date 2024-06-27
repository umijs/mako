const root = require("./root.js");

it("can solve conflict in local scope", () => {
  expect(root.makeModels()).toStrictEqual([42]);
});
