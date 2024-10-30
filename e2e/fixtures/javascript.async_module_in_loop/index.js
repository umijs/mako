import {displayConfig} from "./component";

it("should require looped async moule", () => {
    expect(displayConfig()).toStrictEqual(["key"])
})

