import React, { useState, useEffect } from 'react';
import ReactDOM from 'react-dom/client';

const worker = new Worker('./worker.ts');

function App() {
  const [data, setData] = useState(0);

  useEffect(() => {
    worker.addEventListener('message', (message) => {
      setData(message.data);
    });
  }, []);

  function onClick() {
    worker.postMessage({ command: 'start' });
  }

  return (
    <div>
      <button onClick={onClick}>Click</button>
      <div>Calculate result: {data}</div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
