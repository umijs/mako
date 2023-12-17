export enum x {
	a = 3,
	b = 6,
}
import { x } from './enum-constants'
console.log([
	+x.b,
	-x.b,
	~x.b,
	!x.b,
	typeof x.b,
], [
	x.a + x.b,
	x.a - x.b,
	x.a * x.b,
	x.a / x.b,
	x.a % x.b,
	x.a ** x.b,
], [
	x.a < x.b,
	x.a > x.b,
	x.a <= x.b,
	x.a >= x.b,
	x.a == x.b,
	x.a != x.b,
	x.a === x.b,
	x.a !== x.b,
], [
	x.b << 1,
	x.b >> 1,
	x.b >>> 1,
], [
	x.a & x.b,
	x.a | x.b,
	x.a ^ x.b,
], [
	x.a && x.b,
	x.a || x.b,
	x.a ?? x.b,
])
export const a = 3
export const b = 6