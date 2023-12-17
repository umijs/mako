function empty1() {}
function empty2() {}
function empty3() {}

function identity1(x) { return x }
function identity2(x) { return x }
function identity3(x) { return x }

check(
	empty1(),
	empty2(args),
	empty3(...args),

	identity1(),
	identity2(args),
	identity3(...args),
)
export function empty1() {}
export function empty2() {}
export function empty3() {}

export function identity1(x) { return x }
export function identity2(x) { return x }
export function identity3(x) { return x }
import {
	empty1,
	empty2,
	empty3,

	identity1,
	identity2,
	identity3,
} from './inner.js'

check(
	empty1(),
	empty2(args),
	empty3(...args),

	identity1(),
	identity2(args),
	identity3(...args),
)