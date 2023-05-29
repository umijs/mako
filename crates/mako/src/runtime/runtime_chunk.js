// TODO: window.jsonpCallback([chunkIds, modules]);
// xian gou yi xia
const chunk_modules = {};
function g_define(moduleId, fn) {
  chunk_modules[moduleId] = fn;
}
const registerModulesForChunk = function (fn) {
  fn();
  window.jsonpCallback([['main'], chunk_modules]);
};
registerModulesForChunk(function () {});
