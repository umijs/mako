import assert from 'assert';
import { parseServerStats } from './';

(() => {
  let stats: any = {
    modules: {
      a: { id: 'a', dependents: [] },
      b: { id: 'b', dependents: ['a'] },
      c: { id: 'c', dependents: ['a'] },
    },
    rscClientComponents: [{ path: 'path1', moduleId: 'b' }],
    rscCSSModules: [{ path: 'path2', moduleId: 'c', modules: true }],
  };
  let x = parseServerStats(stats);
  // console.log(JSON.stringify(x, null, 2));
  assert(
    JSON.stringify(x, null, 2) ===
      `
{
  "rscCSSModules": [
    {
      "path": "path2",
      "moduleId": "c",
      "entries": [
        "a"
      ]
    }
  ],
  "rscClientComponents": [
    {
      "path": "path1",
      "moduleId": "b",
      "entries": [
        "a"
      ]
    }
  ]
}
  `.trim(),
  );
})();
