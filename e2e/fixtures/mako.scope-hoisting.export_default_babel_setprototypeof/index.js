import conflict from "./c"
import setPrototypeOf from "./module_fn";

it("should run without error", function() {

	expect(conflict).toBe(2);
	let A = function(){}

	setPrototypeOf(A, {log:function(){console.log(1)}})


	console.log(new A())
});
