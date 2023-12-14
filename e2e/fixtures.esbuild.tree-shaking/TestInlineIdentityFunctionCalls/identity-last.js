function DROP(x) { return x }
console.log(DROP(1))
DROP(foo())
DROP(1)
function DROP(x) { return [x] }
function DROP(x) { return x }
console.log(DROP(1))
DROP(foo())
DROP(1)