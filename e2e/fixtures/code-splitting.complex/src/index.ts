import React from 'react';

console.log(React);

import('./should-be-split').then((m) => console.log(m));
import('./should-be-common').then((m) => console.log(m));
import('./should-be-merged').then((m) => console.log(m));
import('./other-dynamic').then((m) => console.log(m));
