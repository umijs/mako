addEventListener('message', (message) => {
  if ((message.data.command = 'start')) {
    const worker = new Worker(new URL('./worker.ts', import.meta.url));
    worker.addEventListener('message', (message) => {
      self.postMessage(message.data);
    });

    setTimeout(() => {
      worker.postMessage({ command: 'start' });
    }, 1000);
  }
});
