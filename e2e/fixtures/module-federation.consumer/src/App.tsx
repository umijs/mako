import React from 'react';

// @ts-ignore
const RemoteComp = React.lazy(() => import('producer/App'));

const App = () => {
  return (
    <div style={{ display: 'flex', flexDirection: 'column' }}>
      <div
        style={{
          margin: '10px',
          padding: '10px',
          textAlign: 'center',
          backgroundColor: 'greenyellow',
        }}
      >
        <h1>Consumer App</h1>
      </div>
      <React.Suspense>
        <RemoteComp />
      </React.Suspense>
    </div>
  );
};

export default App;
