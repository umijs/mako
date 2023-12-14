function Foo() {}

let a_SHOULD_REMOVED = <div />;
let b_SHOULD_REMOVED = <Foo>{a}</Foo>;
let c_SHOULD_REMOVED = <>{b}</>;

let d = <div />;
let e = <Foo>{d}</Foo>;
let f = <>{e}</>;
console.log(f);
