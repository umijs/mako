import { x } from './ext';
let root = require('./root.js');

it('should import the same value after ext value update',()=>{
	expect(root.x).toBe(x)
})
