// swc will hoist
// working only with react-error-overlay@6.0.9
import * as ErrorOverlay from 'react-error-overlay';

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

  const socket = new WebSocket('ws://__HOST__:__PORT__/__/hmr-ws');
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
