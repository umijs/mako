import * as React from 'react';
import * as ReactDOM from 'react-dom/client';
import './index.css';

const Lazy = React.lazy(async () => {
  // NOTE: 现在还没有集成加载器，先通过延迟解决时序问题
  await new Promise((resolve) => {
    setTimeout(() => {
      resolve();
    }, 1000);
  });
  return await import('./lazy');
});

function App() {
  return (
    <div>
      <Lazy />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
