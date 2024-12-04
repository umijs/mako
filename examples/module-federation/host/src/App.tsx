import React from 'react';
import Widget1 from 'widget/App1';
import Widget2 from 'widget/App2';

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
      <Widget1 />
      <Widget2 />
    </div>
  );
};

export default App;
