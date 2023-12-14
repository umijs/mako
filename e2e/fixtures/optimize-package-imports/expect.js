const assert = require("assert");
const { parseBuildResult, moduleReg } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const names = Object.keys(files).join(",");
const content = files["index.js"];

// assert(content.includes("console.log('hello')"), "should have console.log('hello')");

// TODO:
// 1\ export * as foo from './foo';
// 2\ alias
// 3\ externals
// 4\ sideEffects: false + cjs
// 5\ cjs remix esm

/**
 * Expect:
 * a 和 c 的 import 都应该被替换成子路径
 * b 的 import 都不应该被替换成子路径
 */


