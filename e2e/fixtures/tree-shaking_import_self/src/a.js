// getDiffColor.js
import getDiffColor from './a';

const TypeCodeColor = {};

export default (type) => {
  if (type) {
    return TypeCodeColor[type];
  } else {
    return 'rgba(0,0,0,0)';
  }
};

export function boxShadow(type) {
  return `inset 0 0 0 3px ${getDiffColor(type)}`;
}
