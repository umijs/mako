const p = require('path')


module.exports = [{
  async load(path) {
    if (path.startsWith('virtual:entry')) {
      return {
        content: `import "virtual:file?path=${p.join(__dirname, 'src/virtual.ts')}"`,
        type: 'js',
      };
    }
    if (path.startsWith('virtual:file')) {
      return {
        content: `
import { foo } from "foo";
import { relative } from "./relative";

console.log(foo,relative);

it("can import from dep", ()=>{
    expect(foo).toBe("foo")
})

it("can import from relative", ()=>{
    expect(relative).toBe("relative")
})


`,
        type: 'js',
      };
    }
  }
}];
