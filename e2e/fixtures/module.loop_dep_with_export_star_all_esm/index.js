import * as theExportsAll from "./loop.index";
import * as replicatedExportsAll from "./b.js";

console.log(replicatedExportsAll);

it("should keep all export loop", function () {
  expect(replicatedExportsAll.c).toBe(theExportsAll.c);
});
