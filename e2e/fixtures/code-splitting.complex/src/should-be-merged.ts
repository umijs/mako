import React from 'react';
import context from './context';

const isolated = React.lazy(() => import('./isolated'));
console.log(React, context, isolated);

export default 1;
