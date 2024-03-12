import React from 'react';
import Button from 'antd/es/button';

console.log(React, Button);

import('./should-be-split').then((m) => console.log(m));
import('./should-not-be-common').then((m) => console.log(m));
import('./should-be-merged').then((m) => console.log(m));
import('./other-dynamic').then((m) => console.log(m));
