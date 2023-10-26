addEventListener('message', (message) => {
  if ((message.data.command = 'start')) {
    const result = calculate();
    postMessage(result);
  }
});

function calculate() {
  return 100;
}
