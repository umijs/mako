import("./foo").then(() => {
  console.log("foo loaded");
});

import("foo").then(() => {
  console.log("foo module loaded");
});

async function fn() {
  const lazy = await import('./lazy.ts')
  console.log(lazy)
}
fn()
