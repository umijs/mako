import a from "./module_fn";
import A from "./module_class";

it("should have the correct values", function() {
	expect(a()).toBe("default");
	expect(new A().m()).toBe("A");
});
