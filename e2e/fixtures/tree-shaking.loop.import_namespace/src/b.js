import * as a from "./a"

export const foo = "foo";


expect(Object.keys(a).sort()).toStrictEqual(['a','b','foo'])
