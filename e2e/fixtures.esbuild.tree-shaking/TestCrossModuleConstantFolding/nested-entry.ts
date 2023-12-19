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
import { a, b } from './const-constants'
console.log([
	+b,
	-b,
	~b,
	!b,
	typeof b,
], [
	a + b,
	a - b,
	a * b,
	a / b,
	a % b,
	a ** b,
], [
	a < b,
	a > b,
	a <= b,
	a >= b,
	a == b,
	a != b,
	a === b,
	a !== b,
], [
	b << 1,
	b >> 1,
	b >>> 1,
], [
	a & b,
	a | b,
	a ^ b,
], [
	a && b,
	a || b,
	a ?? b,
])
export const a = 2
export const b = 4
export const c = 8
export enum x {
	a = 16,
	b = 32,
	c = 64,
}
import { a, b, c, x } from './nested-constants'
console.log({
	'should be 4': ~(~a & ~b) & (b | c),
	'should be 32': ~(~x.a & ~x.b) & (x.b | x.c),
})