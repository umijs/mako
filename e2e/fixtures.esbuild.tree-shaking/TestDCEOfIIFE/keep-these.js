(() => {})();
(() => {})(keepThisButRemoveTheIIFE);
(() => { /* @__PURE__ */ removeMe() })();
var someVar;
(x => {})(someVar);
undef = (() => {})();
(() => { keepMe() })();
((x = keepMe()) => {})();
var someVar;
(([y]) => {})(someVar);
(({z}) => {})(someVar);