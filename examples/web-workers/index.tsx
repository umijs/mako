import React, { useState, useEffect } from 'react';
import ReactDOM from 'react-dom/client';

const worker = new Worker(new URL('./worker.ts', import.meta.url));
// nested worker
// const worker = new Worker(new URL('./nestedWorker.ts', import.meta.url));

function App() {
  const [data, setData] = useState<number | null>(null);
  const [working, setWorking] = useState(false);

  useEffect(() => {
    worker.addEventListener('message', (message) => {
      setWorking(false);
      setData(message.data);
    });
  }, []);

  function onClick() {
    setWorking(true);
    worker.postMessage({ command: 'start' });
  }

  return (
    <div>
      <button onClick={onClick}>Click to calculate</button>
      <div>
        <h2>Worker</h2>
        {working && 'Calculating...'}
        {!working && data && `Calculate result: ${data}`}
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
