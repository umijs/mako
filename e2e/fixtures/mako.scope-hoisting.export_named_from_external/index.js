import * as ns from "./root"

it("should have the correct values", function() {
	expect(ns).toStrictEqual({
		foo: 1,
		bar: 'bar',
		qux: 'qux'
	});
});
