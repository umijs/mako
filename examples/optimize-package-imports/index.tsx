import React from 'react';
import ReactDOM from 'react-dom/client';
import { a, b } from './common';

function App() {
  return (
    <div>
      {a} --- {b}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
