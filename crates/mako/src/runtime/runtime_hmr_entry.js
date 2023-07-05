(function () {
  const socket = new WebSocket('ws://__HOST__:__PORT__/__/hmr-ws');
  let latestHash = '';
  let updating = false;

  function runHotUpdate() {
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
    console.log(rawMessage);
    const msg = JSON.parse(rawMessage.data);
    latestHash = msg.hash;

    if (!updating) {
      runHotUpdate();
    }
  });
})();
