import {createRequire as __internalCreateRequire, Module as __internalModule } from "node:module";
const require = __internalCreateRequire(import.meta.url);
const { dirname } = import.meta;
let mod;
if (import.meta.main) {
    mod = __internalModule._load(`${dirname}/example.js`, null, true)
} else {
    mod = require(`${dirname}/example.js`);
}
export default mod;
const __deno_export_1__ = mod;
export { __deno_export_1__ as 'module.exports' };