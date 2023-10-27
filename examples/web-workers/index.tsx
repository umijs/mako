import React, { useState, useEffect } from 'react';
import ReactDOM from 'react-dom/client';

// 普通的 worker
const commonWorker = new Worker('./commonWorker.ts');
// 嵌套 worker
const nestedWorker = new Worker('./nestedWorker.ts');
// new URL
const urlWorker = new Worker(new URL('./commonWorker.ts', import.meta.url));

function App() {
  const [commonData, setCommonData] = useState<number | null>(null);
  const [nestedData, setNestedData] = useState<number | null>(null);
  const [urlData, setURLData] = useState<number | null>(null);

  const [commonWorking, setCommonWorking] = useState(false);
  const [nestedWoring, setNestedWoring] = useState(false);
  const [urlWorking, setURLWorking] = useState(false);

  useEffect(() => {
    commonWorker.addEventListener('message', (message) => {
      setCommonWorking(false);
      setCommonData(message.data);
    });

    nestedWorker.addEventListener('message', (message) => {
      setNestedWoring(false);
      setNestedData(message.data);
    });

    urlWorker.addEventListener('message', (message) => {
      setURLWorking(false);
      setURLData(message.data);
    });
  }, []);

  function onClick() {
    setCommonWorking(true);
    setNestedWoring(true);
    setURLWorking(true);
    commonWorker.postMessage({ command: 'start' });
    nestedWorker.postMessage({ command: 'start' });
    urlWorker.postMessage({ command: 'start' });
  }

  return (
    <div>
      <button onClick={onClick}>Click to calculate</button>

      <div>
        <h2>Common Worker</h2>
        {commonWorking && 'Calculating...'}
        {!commonWorking && commonData && `Calculate result: ${commonData}`}
      </div>

      <div>
        <h2>Nested Worker</h2>
        {nestedWoring && 'Calculating...'}
        {!nestedWoring && nestedData && `Calculate result: ${nestedData}`}
      </div>

      <div>
        <h2>URL Worker</h2>
        {urlWorking && 'Calculating...'}
        {!urlWorking && urlData && `Calculate result: ${urlData}`}
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
