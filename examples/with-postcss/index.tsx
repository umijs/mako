import React from 'react';
import ReactDOM from 'react-dom/client';
import './index.less';
import './index.css';

function App() {
  return (
    <div>
      <h1>
        Hello, <span className="blue">Less</span>
      </h1>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
