// Keep the override scoped to pack: other NAPI packages must load their own bindings.
const Module = require("node:module");
const path = require("node:path");

const nativePath = process.env.UTOO_PACK_NATIVE_BINDING_PATH;
if (nativePath) {
  const bindingPath = path.resolve(nativePath);
  const loadJavaScript = Module._extensions[".js"];
  Module._extensions[".js"] = function (module, filename) {
    const normalizedPath = filename.replace(/\\/g, "/");
    if (/\/@utoo\/pack\/(?:cjs|esm)\/binding\.js$/.test(normalizedPath)) {
      module.exports = module.require(bindingPath);
      return;
    }
    return loadJavaScript(module, filename);
  };
}
