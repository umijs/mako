import { a as b } from "./file1";

let x = (() => {
  function a() {
    b();
    return "ok-root";
  }
  const a_1 = "conflict";

  console.log(a_1);
  return a;
})();

let c = {
  a: x,
  b,
};

export { c };
