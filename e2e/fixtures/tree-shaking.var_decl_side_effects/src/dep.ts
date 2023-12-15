export function used() {
  console.log('used');
}

let declareWithSideEffects1 = console.log(1);
let declareWithSideEffects2 = new Array(1);
let declareWithSideEffects3 = new Regex(/x/);

export { declareWithSideEffects1, declareWithSideEffects2, declareWithSideEffects3 };

let declareWithOutSideEffects1 = 1;
let declareWithOutSideEffects2 = Math.random();
let declareWithOutSideEffects3 = /regex/;

export { declareWithOutSideEffects1, declareWithOutSideEffects2, declareWithOutSideEffects3 };
