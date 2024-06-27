const root = require("./root");

// to make thess modules as external
import("./at_stmt_2");
import("./at_stmt_3");

it("should export all exports from inner", function () {
  expect(root).toStrictEqual({
    a: "named export first",
    b: "from stmt 1",
    c: "from stmt 1",
    d: "from stmt 2",
    e: "from stmt 2",
    f: "from stmt 3",
  });
});
