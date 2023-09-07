import React from 'react';

console.log(React);

import('./should-be-split').then((m) => console.log(m));
import('./should-be-merged').then((m) => console.log(m));
