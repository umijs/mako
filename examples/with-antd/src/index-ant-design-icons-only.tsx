import React from 'react';
import ReactDOM from 'react-dom/client';
import { AntDesignIcons } from './pages/ant-design-icons';

function App() {
  return (
    <div data-test-id="app">
      <AntDesignIcons />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
