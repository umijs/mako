import * as theExportsAll from "./loop.index";
import * as replicatedExportsAll from "./b.js";

it("will miss cjs export in loop exports all", function () {
  expect(theExportsAll.c).toBe("c");
  expect(replicatedExportsAll.c).toBe(undefined);
});
