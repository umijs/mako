function DROP() {}
console.log(DROP(foo(), bar()))
console.log(DROP(foo(), 1))
console.log(DROP(1, foo()))
console.log(DROP(1))
console.log(DROP())
DROP(foo(), bar())
DROP(foo(), 1)
DROP(1, foo())
DROP(1)
DROP()
function DROP() {}
console.log((DROP(), DROP(), foo()))
console.log((DROP(), foo(), DROP()))
console.log((foo(), DROP(), DROP()))
for (DROP(); DROP(); DROP()) DROP();
DROP(), DROP(), foo();
DROP(), foo(), DROP();
foo(), DROP(), DROP();
function DROP() {}
if (foo) { let bar = baz(); bar(); bar() } else DROP();
function DROP() { return x }
function DROP() { return }
console.log(DROP())
DROP()
import { DROP } from './empty-cross-module-def'
console.log(DROP())
DROP()