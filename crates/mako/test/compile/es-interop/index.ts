import cjs from './cjs';
import { foo } from './cjs';
import * as cjsNs from './cjs';
console.log(cjs.foo(1));
console.log(foo(1));
console.log(cjsNs.foo(1));

import { foo as foo2 } from './reexports';
console.log(foo2(1));
