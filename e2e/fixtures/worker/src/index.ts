const worker = new Worker('./worker.ts');
const worker2 = new Worker(new URL('./worker2.ts', import.meta.url));

worker.postMessage({ command: 'start' });
worker2.postMessage({ command: 'start' });

worker.addEventListener('message', (message) => {
  console.log('worker message data:', message.data);
});

worker2.addEventListener('message', (message) => {
  console.log('worker2 message data:', message.data);
});
