import {getEnumFoo} from "./enum"

it("should return the enum value",()=>{

	expect(getEnumFoo()).toBe('foo');

})
