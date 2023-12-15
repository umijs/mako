import("./foo").then(() => {
  console.log("foo loaded");
});

import("foo").then(() => {
  console.log("foo module loaded");
});
