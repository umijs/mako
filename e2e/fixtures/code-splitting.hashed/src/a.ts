import utils from './utils';

// @ts-ignore
import('./b.css').then(() => {
  document.getElementById('root')!.innerHTML = `a ${utils}`;
});

export default 'a';
