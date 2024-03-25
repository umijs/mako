//ref https://www.unpkg.com/browse/@babel/runtime@7.23.9/helpers/esm/setPrototypeOf.js
export default function _setPrototypeOf(o, p) {
  _setPrototypeOf = Object.setPrototypeOf ? Object.setPrototypeOf.bind() : function _setPrototypeOf(o, p) {
    o.__proto__ = p;
    return o;
  };
  return _setPrototypeOf(o, p);
}