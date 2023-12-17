export const foo = 1;
const bar = 2;
export { bar };

export const zoo = 1;

if (true) {
  const foo = 2;
  console.log(bar);
}
