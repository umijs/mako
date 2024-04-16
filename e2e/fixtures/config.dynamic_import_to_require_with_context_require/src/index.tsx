import("./foo").then(() => {
  console.log("foo loaded");
});
import("foo").then(() => {
  console.log("foo module loaded");
});
const a = 'a';
import("./locales/" + a).then(() => {});
