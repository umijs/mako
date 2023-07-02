import { _Z4facti } from './factorial.wasm';

console.log('---- Sync Wasm Module');
const factorial = _Z4facti;
console.log(factorial); // [native code]
console.log(factorial(1));
console.log(factorial(2));
console.log(factorial(3));

import('./factorial.wasm').then(({ _Z4facti: AsyncFactorial }) => {
  console.log('---- Async Wasm Module');
  console.log(AsyncFactorial); // [native code]
  console.log(AsyncFactorial(1));
  console.log(AsyncFactorial(2));
  console.log(AsyncFactorial(3));
});
