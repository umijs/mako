import React from 'react';
import ReactDOM from 'react-dom/client';
import './index.scss';

function App() {
  return (
    <div>
      <h1>
        Hello, <span className="blue">Sass</span>
      </h1>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
