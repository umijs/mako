var makoQueues =
  typeof Symbol === 'function' ? Symbol('mako queues') : '__mako_queues__';
var makoExports =
  typeof Symbol === 'function' ? Symbol('mako exports') : 'exports';
var makoError =
  typeof Symbol === 'function' ? Symbol('mako error') : '__mako_error__';
var resolveQueue = (queue) => {
  if (queue && queue.d < 1) {
    queue.d = 1;
    queue.forEach((fn) => fn.r--);
    queue.forEach((fn) => (fn.r-- ? fn.r++ : fn()));
  }
};
var wrapDeps = (deps) =>
  deps.map((dep) => {
    if (dep !== null && typeof dep === 'object') {
      if (dep[makoQueues]) return dep;
      if (dep.then) {
        var queue = [];
        queue.d = 0;
        dep.then(
          (r) => {
            obj[makoExports] = r;
            resolveQueue(queue);
          },
          (e) => {
            obj[makoError] = e;
            resolveQueue(queue);
          },
        );
        var obj = {};
        obj[makoQueues] = (fn) => fn(queue);
        return obj;
      }
    }
    var ret = {};
    ret[makoQueues] = (x) => {};
    ret[makoExports] = dep;
    return ret;
  });
requireModule.async = (module, body, hasAwait) => {
  var queue;
  hasAwait && ((queue = []).d = -1);
  var depQueues = new Set();
  var exports = module.exports;
  var currentDeps;
  var outerResolve;
  var reject;
  var promise = new Promise((resolve, rej) => {
    reject = rej;
    outerResolve = resolve;
  });
  promise[makoExports] = exports;
  promise[makoQueues] = (fn) => (
    queue && fn(queue), depQueues.forEach(fn), promise['catch']((x) => {})
  );
  module.exports = promise;
  body(
    (deps) => {
      currentDeps = wrapDeps(deps);
      var fn;
      var getResult = () =>
        currentDeps.map((d) => {
          if (d[makoError]) throw d[makoError];
          return d[makoExports];
        });
      var promise = new Promise((resolve) => {
        fn = () => resolve(getResult);
        fn.r = 0;
        var fnQueue = (q) =>
          q !== queue &&
          !depQueues.has(q) &&
          (depQueues.add(q), q && !q.d && (fn.r++, q.push(fn)));
        currentDeps.map((dep) => dep[makoQueues](fnQueue));
      });
      return fn.r ? promise : getResult();
    },
    (err) => (
      err ? reject((promise[makoError] = err)) : outerResolve(exports),
      resolveQueue(queue)
    ),
  );
  queue && queue.d < 0 && (queue.d = 0);
};
