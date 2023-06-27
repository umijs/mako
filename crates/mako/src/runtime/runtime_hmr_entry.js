const socket = new WebSocket('ws://__HOST__:__PORT__/__/hmr-ws');
socket.addEventListener('message', () => {
  module.hot.check().catch((e) => {
    console.error('[HMR] HMR check failed', e);
  });
});
