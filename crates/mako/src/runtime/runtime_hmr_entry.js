// swc will hoist
const RefreshRuntime = require('react-refresh');
RefreshRuntime.injectIntoGlobalHook(self);
self.$RefreshReg$ = () => {};
self.$RefreshSig$ = () => (type) => type;

(function () {
  let hadRuntimeError = false;

  function startReportingRuntimeErrors(options) {
    const errorHandler = () => {
      options.onError();
      hadRuntimeError = true;
    };
    window.addEventListener('error', errorHandler);
    window.addEventListener('unhandledrejection', errorHandler);
    return () => {
      window.removeEventListener('error', errorHandler);
      window.removeEventListener('unhandledrejection', errorHandler);
    };
  }

  const stopReportingRuntimeError = startReportingRuntimeErrors({
    onError: function () {
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
    const h = getHost();
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
      return Promise.all([module.hot.check(), module.hot.updateChunksUrlMap()])
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
