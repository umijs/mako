import React from 'react';
import ReactDOM from 'react-dom/client';
import Button from 'antd/es/button/index.js';

function App() {
  return (
    <div>
      <Button> Antd </Button>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
