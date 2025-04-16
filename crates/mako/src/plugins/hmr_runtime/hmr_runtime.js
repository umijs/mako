//
'use strict';
!(function () {
  requireModule._h = '_%full_hash%_';
  requireModule.currentHash = function () {
    return requireModule._h;
  };
})();
!(function () {
  var currentParents = [];
  var currentChildModule;
  requireModule.hmrC = {};
  var createHmrRequire = function (require, moduleId) {
    var me = modulesRegistry[moduleId];
    if (!me) return require;
    var fn = function (request) {
      if (me.hot.active) {
        if (modulesRegistry[request]) {
          var parents = modulesRegistry[request].parents;
          if (!parents.includes(moduleId)) {
            parents.push(moduleId);
          }
        } else {
          currentParents = [moduleId];
          currentChildModule = request;
        }
        if (!me.children.includes(request)) {
          me.children.push(request);
        }
      } else {
      }
      return require(request);
    };
    var createPropertyDescriptor = function createPropertyDescriptor(name) {
      return {
        configurable: true,
        enumerable: true,
        get: function get() {
          return require[name];
        },
        set: function set(value) {
          require[name] = value;
        },
      };
    };
    for (var name in require) {
      if (Object.prototype.hasOwnProperty.call(require, name)) {
        Object.defineProperty(fn, name, createPropertyDescriptor(name));
      }
    }
    return fn;
  };
  var applyHotUpdate = function (_chunkId, update) {
    var modules = update.modules,
      removedModules = update.removedModules;
    var outdatedModules = [];
    var _iteratorNormalCompletion = true,
      _didIteratorError = false,
      _iteratorError = undefined;
    try {
      for (
        var _iterator = Object.keys(modules)[Symbol.iterator](), _step;
        !(_iteratorNormalCompletion = (_step = _iterator.next()).done);
        _iteratorNormalCompletion = true
      ) {
        var moduleId = _step.value;
        if (!modulesRegistry[moduleId]) continue;
        if (outdatedModules.includes(moduleId)) continue;
        outdatedModules.push(moduleId);
        var queue = [moduleId];
        while (queue.length) {
          var item = queue.pop();
          var module = modulesRegistry[item];
          if (!module) continue;
          if (module.hot._main) {
            location.reload();
          }
          if (module.hot._selfAccepted) {
            continue;
          }
          var _iteratorNormalCompletion1 = true,
            _didIteratorError1 = false,
            _iteratorError1 = undefined;
          try {
            for (
              var _iterator1 = module.parents[Symbol.iterator](), _step1;
              !(_iteratorNormalCompletion1 = (_step1 = _iterator1.next()).done);
              _iteratorNormalCompletion1 = true
            ) {
              var parentModule = _step1.value;
              if (outdatedModules.includes(parentModule)) continue;
              outdatedModules.push(parentModule);
              queue.push(parentModule);
            }
          } catch (err) {
            _didIteratorError1 = true;
            _iteratorError1 = err;
          } finally {
            try {
              if (!_iteratorNormalCompletion1 && _iterator1.return != null) {
                _iterator1.return();
              }
            } finally {
              if (_didIteratorError1) {
                throw _iteratorError1;
              }
            }
          }
        }
      }
    } catch (err) {
      _didIteratorError = true;
      _iteratorError = err;
    } finally {
      try {
        if (!_iteratorNormalCompletion && _iterator.return != null) {
          _iterator.return();
        }
      } finally {
        if (_didIteratorError) {
          throw _iteratorError;
        }
      }
    }
    var outdatedSelfAcceptedModules = [];
    var _iteratorNormalCompletion2 = true,
      _didIteratorError2 = false,
      _iteratorError2 = undefined;
    try {
      for (
        var _iterator2 = outdatedModules[Symbol.iterator](), _step2;
        !(_iteratorNormalCompletion2 = (_step2 = _iterator2.next()).done);
        _iteratorNormalCompletion2 = true
      ) {
        var moduleId1 = _step2.value;
        var module1 = modulesRegistry[moduleId1];
        if (module1.hot._selfAccepted) {
          outdatedSelfAcceptedModules.push(module1);
        }
      }
    } catch (err) {
      _didIteratorError2 = true;
      _iteratorError2 = err;
    } finally {
      try {
        if (!_iteratorNormalCompletion2 && _iterator2.return != null) {
          _iterator2.return();
        }
      } finally {
        if (_didIteratorError2) {
          throw _iteratorError2;
        }
      }
    }
    var _iteratorNormalCompletion3 = true,
      _didIteratorError3 = false,
      _iteratorError3 = undefined;
    try {
      for (
        var _iterator3 = outdatedModules[Symbol.iterator](), _step3;
        !(_iteratorNormalCompletion3 = (_step3 = _iterator3.next()).done);
        _iteratorNormalCompletion3 = true
      ) {
        var moduleId2 = _step3.value;
        var module2 = modulesRegistry[moduleId2];
        var _iteratorNormalCompletion4 = true,
          _didIteratorError4 = false,
          _iteratorError4 = undefined;
        try {
          for (
            var _iterator4 = module2.hot._disposeHandlers[Symbol.iterator](),
              _step4;
            !(_iteratorNormalCompletion4 = (_step4 = _iterator4.next()).done);
            _iteratorNormalCompletion4 = true
          ) {
            var handler = _step4.value;
            handler();
          }
        } catch (err) {
          _didIteratorError4 = true;
          _iteratorError4 = err;
        } finally {
          try {
            if (!_iteratorNormalCompletion4 && _iterator4.return != null) {
              _iterator4.return();
            }
          } finally {
            if (_didIteratorError4) {
              throw _iteratorError4;
            }
          }
        }
        module2.hot.active = false;
        delete modulesRegistry[moduleId2];
        var _iteratorNormalCompletion5 = true,
          _didIteratorError5 = false,
          _iteratorError5 = undefined;
        try {
          for (
            var _iterator5 = module2.children[Symbol.iterator](), _step5;
            !(_iteratorNormalCompletion5 = (_step5 = _iterator5.next()).done);
            _iteratorNormalCompletion5 = true
          ) {
            var childModule = _step5.value;
            var child = modulesRegistry[childModule];
            if (!child) continue;
            var idx = child.parents.indexOf(moduleId2);
            if (idx !== -1) {
              child.parents.splice(idx, 1);
            }
          }
        } catch (err) {
          _didIteratorError5 = true;
          _iteratorError5 = err;
        } finally {
          try {
            if (!_iteratorNormalCompletion5 && _iterator5.return != null) {
              _iterator5.return();
            }
          } finally {
            if (_didIteratorError5) {
              throw _iteratorError5;
            }
          }
        }
      }
    } catch (err) {
      _didIteratorError3 = true;
      _iteratorError3 = err;
    } finally {
      try {
        if (!_iteratorNormalCompletion3 && _iterator3.return != null) {
          _iterator3.return();
        }
      } finally {
        if (_didIteratorError3) {
          throw _iteratorError3;
        }
      }
    }
    registerModules(modules);
    var _iteratorNormalCompletion6 = true,
      _didIteratorError6 = false,
      _iteratorError6 = undefined;
    try {
      for (
        var _iterator6 = outdatedSelfAcceptedModules[Symbol.iterator](), _step6;
        !(_iteratorNormalCompletion6 = (_step6 = _iterator6.next()).done);
        _iteratorNormalCompletion6 = true
      ) {
        var module3 = _step6.value;
        module3.hot._requireSelf();
      }
    } catch (err) {
      _didIteratorError6 = true;
      _iteratorError6 = err;
    } finally {
      try {
        if (!_iteratorNormalCompletion6 && _iterator6.return != null) {
          _iterator6.return();
        }
      } finally {
        if (_didIteratorError6) {
          throw _iteratorError6;
        }
      }
    }
  };
  var createModuleHotObject = function (moduleId, me) {
    var _main = currentChildModule !== moduleId;
    var hot = {
      _acceptedDependencies: {},
      _declinedDependencies: {},
      _selfAccepted: false,
      _selfDeclined: false,
      _selfInvalidated: false,
      _disposeHandlers: [],
      _requireSelf: function _requireSelf() {
        currentParents = me.parents.slice();
        currentChildModule = _main ? undefined : moduleId;
        requireModule(moduleId);
      },
      _main: _main,
      active: true,
      accept: function () {
        this._selfAccepted = true;
      },
      dispose: function (callback) {
        this._disposeHandlers.push(callback);
      },
      invalidate: function () {},
      updateChunksUrlMap: function () {
        var current_hash = requireModule.currentHash();
        return fetch(
          ''
            .concat(requireModule.publicPath)
            .concat(current_hash, '.hot-update-url-map.json'),
        )
          .then(function (res) {
            return res.json();
          })
          .then(function (chunksUrlMap) {
            Object.assign(chunksIdToUrlMap, chunksUrlMap.js);
            Object.assign(cssChunksIdToUrlMap, chunksUrlMap.css);
          });
      },
      check: function () {
        var current_hash = requireModule.currentHash();
        return fetch(
          ''
            .concat(requireModule.publicPath)
            .concat(current_hash, '.hot-update.json'),
        )
          .then(function (res) {
            return res.json();
          })
          .then(function (update) {
            return Promise.all(
              update.c.map(function (chunk) {
                var parts = chunk.split('.');
                var l = parts.length;
                var left = parts.slice(0, parts.length - 1).join('.');
                var ext = parts[l - 1];
                var hotChunkName = [left, current_hash, 'hot-update', ext].join(
                  '.',
                );
                return new Promise(function (done) {
                  var url = ''
                    .concat(requireModule.publicPath)
                    .concat(hotChunkName);
                  requireModule.loadScript(url, done);
                });
              }),
            );
          });
      },
      apply: function (update) {
        return applyHotUpdate(update);
      },
    };
    currentChildModule = undefined;
    return hot;
  };
  requireModule.hmrC.jsonp = function (chunkId, update, promises) {
    promises.push(
      new Promise(function (resolve) {
        applyHotUpdate(chunkId, update);
        resolve();
      }),
    );
  };
  requireModule.hmrC.css = function (chunkId, _update, promises) {
    if (cssChunksIdToUrlMap[chunkId]) {
      promises.push(
        new Promise(function (resolve, reject) {
          var url = cssChunksIdToUrlMap[chunkId];
          var fullUrl = requireModule.publicPath + url;
          var oldLink = requireModule.findStylesheet(url);
          if (oldLink) {
            var newLink = requireModule.createStylesheet(
              chunkId,
              ''.concat(fullUrl, '?').concat(Date.now()),
              oldLink,
              function () {
                newLink.rel = 'stylesheet';
                newLink.as = null;
                oldLink.parentNode.removeChild(oldLink);
                resolve();
              },
              reject,
            );
            newLink.rel = 'prereload';
            newLink.as = 'style';
          }
        }),
      );
    }
  };
  requireModule.requireInterceptors.push(function (options) {
    var originRequire = options.require;
    options.module.hot = createModuleHotObject(options.id, options.module);
    options.module.meta = {
      hot: options.module.hot,
    };
    options.module.parents = currentParents;
    currentParents = [];
    options.module.children = [];
    options.require = createHmrRequire(options.require, options.id);
    options.require.currentHash = function () {
      return originRequire._h;
    };
  });
  requireModule.applyHotUpdate = function (chunkId, update, runtime) {
    runtime(requireModule);
    return Promise.all(
      Object.keys(requireModule.hmrC).reduce(function (promises, key) {
        requireModule.hmrC[key](chunkId, update, promises);
        return promises;
      }, []),
    );
  };
})();
