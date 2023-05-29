import * as React from 'react';
import * as ReactDOM from 'react-dom/client';
import './index.css';

const Lazy = React.lazy(async () => {
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
