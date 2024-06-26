import React from 'react';
import context from './context';

const noIncoming = React.lazy(() => import('./no-incoming'));
console.log(React, context, noIncoming);

export default 1;
