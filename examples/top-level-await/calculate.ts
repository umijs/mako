async function wait(time = 1000) {
  await new Promise((resolve) => {
    setTimeout(() => {
      resolve(null);
    }, time);
  });
}

export async function calculate(a: number, b: number) {
  await wait();
  return a + b;
}
