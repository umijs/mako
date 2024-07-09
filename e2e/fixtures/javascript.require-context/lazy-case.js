let context = require.context(
  "./lazy-context",
  false,
  /^\.\/(A|B)\.js$/i,
  "lazy",
);

it("lazy: no sub directorie", () => {
  it("  should get list of context files", () => {
    expect(context.keys().sort()).toStrictEqual(["./a.js", "./b.js"]);
  });

  it("  can require", () => {
    expect(context.resolve("./a.js")).toBe("lazy-context/a.js");

    expect(context("./a.js")).resolves.toStrictEqual({
      default: "async:a.js",
    });
  });

  it("  follow webpack id convention", () => {
    expect(context.id).toBe("./lazy-context/ lazy nonrecursive ^./(A|B).js$/");
  });
});

let context2 = require.context("./lazy-context", true, /\.js$/, "lazy");

it("lazy: with sub directories", () => {
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
    expect(context2.id).toBe("./lazy-context/ lazy .js$/");
  });
});

it("throws when resolve unknow request", () => {
  expect(() => {
    context.resolve("./not_exists.js");
  }).toThrow("Cannot find module './not_exists.js'");
});

it("rejects when require unknow request", () => {
  expect(() => {
    return context("./not_exists.js");
  }).rejects.toThrow("Cannot find module './not_exists.js'");
});
