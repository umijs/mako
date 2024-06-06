import { set } from './ext.js';

export function changeX() {
  set(999);
}

export { x as z } from './ext.js';
