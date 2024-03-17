const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const indexJsContent = files["index.js"];
const jsxFnNodeModulesMapContent = files["jsxFnNodeModulesMap.js"];
const jsxFnIndexContent = files["jsxFnIndex.js"]
const jsxArrowIndex1 = files["jsxArrowIndex1.js"]
assert(
  !indexJsContent.includes("Component$$"),
  "not support js"
);
assert(!jsxFnNodeModulesMapContent.includes("Component$$"), "should not have node_modules")
assert(jsxFnIndexContent.includes("Component$$1") && jsxFnIndexContent.includes("Component$$"), "support modify a repeating variable and support tsx");
assert(jsxArrowIndex1.includes("Component$$") , "support covert arrow function");
// TODO: 暂时没有覆盖箭头函数导出重复变量的问题
