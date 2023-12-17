console.log(require('react'));

(() => {
  function require(a) {
    console.log(a);
  }
  require(1);
})();

(() => {
  function require() {
    console.log(1);
  }
  require();
})();

(() => {
  console.log(require('react'));
})();
