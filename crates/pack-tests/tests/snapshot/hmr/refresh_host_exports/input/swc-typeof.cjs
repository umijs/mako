// @swc/helpers 0.5.15, MIT. Keep its actual constructor probe: an empty helper
// stub would hide the cross-origin failure this fixture is intended to catch.
"use strict";
function _type_of(obj) {
  "@swc/helpers - typeof";
  return obj && typeof Symbol !== "undefined" && obj.constructor === Symbol
    ? "symbol"
    : typeof obj;
}
exports._ = _type_of;
