addEventListener('message', (message) => {
  if ((message.data.command = 'start')) {
    setTimeout(async () => {
      const { result } = await import('./result');
      postMessage(result);
    }, 1000);
  }
});
