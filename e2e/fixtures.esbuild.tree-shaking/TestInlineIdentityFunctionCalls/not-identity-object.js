function DROP(x) { return x }
console.log(DROP(1))
DROP(foo())
DROP(1)
function DROP(x) { return [x] }
function DROP(x) { return x }
console.log(DROP(1))
DROP(foo())
DROP(1)
import { DROP } from './identity-cross-module-def'
console.log(DROP(1))
DROP(foo())
DROP(1)
export function DROP(x) { return x }
function keep(x) { return x }
console.log(keep())
keep()
function keep(x) { return x }
console.log(keep(1, 2))
keep(1, 2)
function keep(x) { return x }
function keep(x) { return [x] }
console.log(keep(1))
keep(foo())
keep(1)
function* keep(x) { return x }
console.log(keep(1))
keep(foo())
keep(1)
async function keep(x) { return x }
console.log(keep(1))
keep(foo())
keep(1)
function keep(x) { return x }
keep = reassigned
console.log(keep(1))
keep(foo())
keep(1)
function keep(x) { return x }
keep++
console.log(keep(1))
keep(foo())
keep(1)
function keep(x) { return x }
keep /= reassigned
console.log(keep(1))
keep(foo())
keep(1)
function keep(x) { return x }
[keep] = reassigned
console.log(keep(1))
keep(foo())
keep(1)
function keep(x) { return x }
({keep} = reassigned)
console.log(keep(1))
keep(foo())
keep(1)
function keep(x, y) { return x }
console.log(keep(1))
keep(foo())
keep(1)
function keep(x = foo()) { return x }
console.log(keep(1))
keep(foo())
keep(1)
function keep([x]) { return x }
console.log(keep(1))
keep(foo())
keep(1)
function keep({x}) { return x }
console.log(keep(1))
keep(foo())
keep(1)