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
      <h2>
        Hello, <span className="blue">Css</span>
      </h2>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
