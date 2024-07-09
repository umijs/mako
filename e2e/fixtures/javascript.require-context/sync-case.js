let context = require.context("./context", false, /^\.\/(A|B)\.js$/i, "sync");

it("sync: no sub directorie", () => {
  it("  should get list of context files", () => {
    expect(context.keys().sort()).toStrictEqual(["./a.js", "./b.js"]);
  });

  it("  should require after resolve", () => {
    expect(context.resolve("./a.js")).toBe("context/a.js");
    expect(require(context.resolve("./a.js"))).toStrictEqual({
      default: "a.js",
    });
  });

  it("  should require directly by context", () => {
    expect(context("./a.js")).toStrictEqual({
      default: "a.js",
    });
  });

  it("  follow webpack id convention", () => {
    expect(context.id).toBe("./context/ sync nonrecursive ^./(A|B).js$/");
  });
});

let context2 = require.context("./context", true, /\.js$/);

it("sync: with sub directories", () => {
  it("  should contains all the js files", () => {
    expect(context2.keys().sort()).toStrictEqual([
      "./a.js",
      "./b.js",
      "./dir/c.js",
      "./dir/d.js",
      "./dir/index.js",
      "./index.js",
    ]);
  });

  it("  follows swebpacl id convetion", () => {
    expect(context2.id).toBe("./context/ sync .js$/");
  });
});

it("throws when resolve unknow request", () => {
  expect(() => {
    context.resolve("./not_exists.js");
  }).toThrow("Cannot find module './not_exists.js'");
});
