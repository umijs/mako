import  def,  {named} from "./inner2";

it("should have the correct values", function() {
	expect(def).toBe("default");
	expect(named).toBe("named");
});
	
