/* @__NO_SIDE_EFFECTS__ */ function f(y) { sideEffect(y) }
/* @__NO_SIDE_EFFECTS__ */ function* g(y) { sideEffect(y) }
f('removeThisCall')
g('removeThisCall')
f(onlyKeepThisIdentifier)
g(onlyKeepThisIdentifier)
x(f('keepThisCall'))
x(g('keepThisCall'))
/* @__NO_SIDE_EFFECTS__ */ const f = function (y) { sideEffect(y) }
/* @__NO_SIDE_EFFECTS__ */ const g = function* (y) { sideEffect(y) }
f('removeThisCall')
g('removeThisCall')
f(onlyKeepThisIdentifier)
g(onlyKeepThisIdentifier)
x(f('keepThisCall'))
x(g('keepThisCall'))