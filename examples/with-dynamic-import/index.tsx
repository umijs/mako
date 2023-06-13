import './render';
const onUpate = () => {
  console.time('hmr');
  module.hot.check().then(() => {
    console.timeEnd('hmr');
  });
};

const socket = new WebSocket('ws://127.0.0.1:3000/__/hmr-ws');

socket.addEventListener('message', onUpate);
