import {ans} from "./inner";
import ignoredDefault, {ignoredNamed} from "pkg"

it("should have the correct values", function() {
	expect(ignoredDefault).toStrictEqual({});
	expect(ignoredNamed).toBe(undefined);
	expect(ans).toBe(42)
});
	
