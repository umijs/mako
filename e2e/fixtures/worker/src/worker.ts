addEventListener('message', async (message) => {
  if (message.data.command = 'start') {
    postMessage('Test worker');
  }
});
