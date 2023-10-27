import React, { useState, useEffect } from 'react';
import ReactDOM from 'react-dom/client';

// 普通的 worker
const worker1 = new Worker('./worker.ts');
// 嵌套 worker
const worker2 = new Worker('./worker2.ts');

function App() {
  const [data1, setData1] = useState<number | null>(null);
  const [working1, setWorking1] = useState(false);

  const [data2, setData2] = useState<number | null>(null);
  const [working2, setWorking2] = useState(false);

  useEffect(() => {
    worker1.addEventListener('message', (message) => {
      setWorking1(false);
      setData1(message.data);
    });

    worker2.addEventListener('message', (message) => {
      setWorking2(false);
      setData2(message.data);
    });
  }, []);

  function onClick() {
    setWorking1(true);
    setWorking2(true);
    worker1.postMessage({ command: 'start' });
    worker2.postMessage({ command: 'start' });
  }

  return (
    <div>
      <button onClick={onClick}>Click to calculate</button>

      {working1 && <div>Calculating...</div>}
      {!working1 && data1 && <div>Calculate result: {data1}</div>}

      {working2 && <div>Calculating...</div>}
      {!working2 && data2 && <div>Calculate result: {data2}</div>}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
