import React from 'react';

function Root(props) {
  const [state, setState] = React.useState(0);
  console.log('state', state);
  return (
    <div>
      <h1>root</h1>
      {props.children}
    </div>
  );
}

export function rootContainer(container) {
  return <Root>{container}</Root>;
}
