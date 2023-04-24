import React, { useState } from 'react';
import ReactDOM from 'react-dom/client';
import Button from 'antd/es/button';
import lodash from 'lodash';
import axios from 'axios';

function App() {
  const [data, setData] = useState(null);

  return (
    <div>
      <Button
        onClick={() => {
          axios.get('https://jsonplaceholder.typicode.com/posts/1').then((res) => {
            setData(res.data);
          });
        }}
      >
        {lodash.toUpper('antd')}{' '}
      </Button>
      <h3>Data from axios</h3>
      <pre>{JSON.stringify(data, null, 2)}</pre>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
