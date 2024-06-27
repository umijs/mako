import React from 'react';
import context from './context';
import './a.less'
const isolated = React.lazy(() => import('./isolated'));
console.log(React, context, isolated);

export default 1;
