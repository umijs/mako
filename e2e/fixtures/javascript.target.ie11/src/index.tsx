import foo from './foo';
console.log(foo.a);
export default () => {
  return foo.a;
}
