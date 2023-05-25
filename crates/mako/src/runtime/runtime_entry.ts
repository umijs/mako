const modules = new Map();
const g_define = (name, moduleFactory) => {
  modules.set(name, moduleFactory);
};
const moduleCache = new Map();
const requireModule = (name) => {
  if (moduleCache.has(name)) {
    return moduleCache.get(name).exports;
  }

  if (!modules.has(name)) {
    throw new Error(`Module '${name}' does not exist.`);
  }

  const moduleFactory = modules.get(name);
  const module = {
    exports: {},
  };
  moduleCache.set(name, module);
  moduleFactory(module, module.exports, requireModule);
  return module.exports;
};
const registerModules = function (fn) {
  fn();
};
registerModules(function () {});
requireModule('main');
