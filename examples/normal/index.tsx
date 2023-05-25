import React from 'react';
import ReactDOM from 'react-dom/client';

import { foo } from './foo';
import './index.css';

function App() {
  return (
    <div>
      <h1>Hello {foo}!</h1>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
