import { fn } from './dep/index.js';

it("the fn should not be undefined", ()=>{
	expect(fn).toBeDefined();
})