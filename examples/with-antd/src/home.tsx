import React, { useState } from 'react';
import Button from 'antd/es/button';
import lodash from 'lodash';
import axios from 'axios';

export function Home() {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(false);

  return (
    <div>
      <h2>Home</h2>
      <Button
        onClick={() => {
          setLoading(true);
          axios
            .get('https://jsonplaceholder.typicode.com/posts/1')
            .then((res) => {
              setData(res.data);
            })
            .finally(() => {
              setLoading(false);
            });
        }}
      >
        {lodash.toUpper('load data')}{' '}
      </Button>
      {loading && <div>loading...</div>}
      {data && <pre>{JSON.stringify(data, null, 2)}</pre>}
    </div>
  );
}
