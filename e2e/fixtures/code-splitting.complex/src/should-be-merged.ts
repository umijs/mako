import React from 'react';
import context from './context';

const vacant = React.lazy(() => import('./vacant'));
console.log(React, context, vacant);

export default 1;
