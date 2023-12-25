// import { AntDesignIcons } from './pages/ant-design-icons';
import { Button } from 'antd';
import React from 'react';
import ReactDOM from 'react-dom/client';

function App() {
  return (
    <div data-test-id="app">
      {/* <AntDesignIcons /> */}
      <Button type="primary">Button</Button>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
