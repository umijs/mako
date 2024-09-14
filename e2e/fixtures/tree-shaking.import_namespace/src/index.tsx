import * as mod from "./mod";

const { shouldKeep2 } = mod

console.log(mod.shouldKeep1(42));
console.log(shouldKeep2(42))

// Guardian don't remove
let array= [1,2,3,]
console.log(array)
let [first, ...rest]  = array
console.log(first,rest)
let copiedArray = [...array]
console.log(copiedArray)

let object = {a: 1,b: 2,c: 3}
console.log(object)
let {a, ...restObject} = object
console.log(a,restObject)
let copiedObject = {...object}
console.log(copiedObject)
