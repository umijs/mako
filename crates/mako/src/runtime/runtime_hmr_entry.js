// swc will hoist
// working only with react-error-overlay@6.0.9
const ErrorOverlay = require('react-error-overlay');
const RefreshRuntime = require('react-refresh');
RefreshRuntime.injectIntoGlobalHook(self);
self.$RefreshReg$ = () => {};
self.$RefreshSig$ = () => (type) => type;

(function () {
  let hadRuntimeError = false;

  const enableErrorOverlay = true;
  enableErrorOverlay &&
    ErrorOverlay.startReportingRuntimeErrors({
      onError: function () {
        hadRuntimeError = true;
      },
    });

  if (module.hot && typeof module.hot.dispose === 'function') {
    module.hot.dispose(function () {
      enableErrorOverlay && ErrorOverlay.stopReportingRuntimeErrors();
    });
  }

  function getHost() {
    if (process.env.SOCKET_SERVER) {
      return new URL(process.env.SOCKET_SERVER);
    }
    return location;
  }

  function getSocketUrl() {
    let h = getHost();
    const host = h.host;
    const isHttps = h.protocol === 'https:';
    return `${isHttps ? 'wss' : 'ws'}://${host}/__/hmr-ws`;
  }

  const socket = new WebSocket(getSocketUrl());

  let latestHash = '';
  let updating = false;

  function runHotUpdate() {
    if (hadRuntimeError) {
      location.reload();
    }

    if (latestHash !== require.currentHash()) {
      updating = true;
      return module.hot
        .check()
        .then(() => {
          updating = false;
          return runHotUpdate();
        })
        .catch((e) => {
          // need a reload?
          console.error('[HMR] HMR check failed', e);
        });
    } else {
      return Promise.resolve();
    }
  }

  socket.addEventListener('message', (rawMessage) => {
    const msg = JSON.parse(rawMessage.data);
    latestHash = msg.hash;

    if (!updating) {
      runHotUpdate();
    }
  });
})();
