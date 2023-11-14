import { result } from './result'; // 默认支持 module 类型的 worker

addEventListener('message', (message) => {
  if ((message.data.command = 'start')) {
    setTimeout(() => {
      postMessage(result);
    }, 1000);
  }
});
