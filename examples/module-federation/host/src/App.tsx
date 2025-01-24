import React from 'react';
// import Widget1 from 'widget/App1';
// import Widget2 from 'widget/App2';

const Widget1 = React.lazy(() => import('widget/App1'));
const Widget2 = React.lazy(() => import('widget/App2'));

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
        <h1>Host App</h1>
      </div>
      <React.Suspense>
        <Widget1 />
      </React.Suspense>
      <React.Suspense>
        <Widget2 />
      </React.Suspense>
    </div>
  );
};

export default App;
