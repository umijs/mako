// swc will hois
'use strict';
// swc will hoist
var RefreshRuntime = require('react-refresh');
RefreshRuntime.injectIntoGlobalHook(self);
self.$RefreshReg$ = function () {};
self.$RefreshSig$ = function () {
  return function (type) {
    return type;
  };
};
(function () {
  var hadRuntimeError = false;
  function startReportingRuntimeErrors(options) {
    var errorHandler = function (e) {
      options.onError(e);
      hadRuntimeError = true;
    };
    window.addEventListener('error', errorHandler);
    // window.addEventListener('unhandledrejection', errorHandler);
    return function () {
      window.removeEventListener('error', errorHandler);
      // window.removeEventListener('unhandledrejection', errorHandler);
    };
  }
  var stopReportingRuntimeError = startReportingRuntimeErrors({
    onError: function (e) {
      console.error(
        '[Mako] Runtime error found, and it will cause a full reload. If you want HMR to work, please fix the error.',
        e,
      );
      hadRuntimeError = true;
    },
  });
  if (module.hot && typeof module.hot.dispose === 'function') {
    module.hot.dispose(function () {
      stopReportingRuntimeError();
    });
  }
  function getHost() {
    if (process.env.SOCKET_SERVER) {
      return new URL(process.env.SOCKET_SERVER);
    }
    return location;
  }
  function getSocketUrl() {
    var h = getHost();
    var host = h.host;
    var isHttps = h.protocol === 'https:';
    return ''.concat(isHttps ? 'wss' : 'ws', '://').concat(host, '/__/hmr-ws');
  }
  var socket = new WebSocket(getSocketUrl());
  var latestHash = '';
  var updating = false;
  function runHotUpdate() {
    if (hadRuntimeError) {
      location.reload();
    }
    if (latestHash !== require.currentHash()) {
      updating = true;
      return Promise.all([module.hot.check(), module.hot.updateChunksUrlMap()])
        .then(function () {
          updating = false;
          return runHotUpdate();
        })
        .catch(function (e) {
          // need a reload?
          console.error('[HMR] HMR check failed', e);
        });
    } else {
      return Promise.resolve();
    }
  }
  socket.addEventListener('message', function (rawMessage) {
    var msg = JSON.parse(rawMessage.data);
    latestHash = msg.hash;
    if (!updating) {
      runHotUpdate();
    }
  });
})();
