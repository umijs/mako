
export async function bar() {
  const foo = await import('./foo');
  return foo.foo + '_bar';
}
