import React from 'react';
export default () => {
  const noIncomingOutgoing = React.lazy(() => import('./no-incoming-outgoing'));
  console.log(noIncomingOutgoing)
  return (<div>
   
    <span>no-incomming-node</span>
  </div>)
}
