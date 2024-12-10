import React, { Suspense } from 'react';

const Lazy = React.lazy(() => import('./lazy'));

const App = () => {
  return (
    <div
      style={{
        margin: '10px',
        padding: '10px',
        textAlign: 'center',
        backgroundColor: 'cyan',
      }}
    >
      <h1>Widget App1</h1>
      <h2>
        <Suspense fallback="loading">
          <Lazy />
        </Suspense>
      </h2>
    </div>
  );
};

export default App;
