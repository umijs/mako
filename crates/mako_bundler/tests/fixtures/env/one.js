function foo() {
  if (process.env.NODE_ENV === 'production') {
    console.log(123);
  }
  if (process.env['NODE_ENV'] === 'production') {
    console.log(123);
  }
  const test = process.env['NODE_ENV'];
}
