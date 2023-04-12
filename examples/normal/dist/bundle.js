
const modules = new Map();
const define = (name, moduleFactory) => {
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
        
define("/Users/chencheng/Documents/Code/github.com/umijs/marko/examples/normal/index.tsx", function(module, exports, require) {
"use strict";
var _foo = require("/Users/chencheng/Documents/Code/github.com/umijs/marko/examples/normal/foo.ts");
var _react = require("react");
var _client = require("react-dom/client");
function App() {
    return React.createElement("div", null, "Hello ", _foo.foo);
}
_client.default.createRoot(document.getElementById("root")).render(React.createElement(App, null));
});
define("react", function(module, exports, require) {
/* external react */ exports.default = React;});
define("react-dom/client", function(module, exports, require) {
/* external react-dom/client */ exports.default = ReactDOM;});
define("/Users/chencheng/Documents/Code/github.com/umijs/marko/examples/normal/foo.ts", function(module, exports, require) {
"use strict";
Object.defineProperty(exports, "foo", {
    enumerable: true,
    get: function() {
        return foo;
    }
});
const foo = "World";
});

requireModule("/Users/chencheng/Documents/Code/github.com/umijs/marko/examples/normal/index.tsx");