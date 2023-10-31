(function makoUniversalModuleDefinition(root, factory) {
  if (typeof exports === 'object' && typeof module === 'object')
    module.exports = factory();
  else if (typeof define === 'function' && define.amd) define([], factory);
  else if (typeof exports === 'object') exports['_%umd_name%_'] = factory();
  else root['_%umd_name%_'] = factory();
})(self, function () {
  return runtime.exports;
});
