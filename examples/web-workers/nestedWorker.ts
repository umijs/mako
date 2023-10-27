addEventListener('message', (message) => {
  if ((message.data.command = 'start')) {
    const worker = new Worker('./commonWorker.ts');
    worker.addEventListener('message', (message) => {
      self.postMessage(message.data);
    });

    setTimeout(() => {
      worker.postMessage({ command: 'start' });
    }, 1000);
  }
});
