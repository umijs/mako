import React from 'react';
import context from './context';

const vancant = React.lazy(() => import('./vancant'));
console.log(React, context, vancant);

export default 1;
