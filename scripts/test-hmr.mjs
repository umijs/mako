import assert from 'assert';
import { execSync } from 'child_process';
import { chromium, devices } from 'playwright';
import waitPort from 'wait-port';
import 'zx/globals';

function skip() {}

const root = process.cwd();
const tmp = path.join(root, 'tmp', 'hmr');
const tmpPackages = path.join(root, 'tmp', 'packages');
if (!fs.existsSync(tmp)) {
  fs.mkdirSync(tmp, { recursive: true });
}
// TODO: check port
const port = 3000;
const DELAY_TIME = parseInt(process.env.DELAY_TIME) || 500;

async function cleanup({ process, browser } = {}) {
  try {
    await killMakoDevServer();
    await browser?.close();
    if (fs.existsSync(tmp)) {
      fs.rmSync(tmp, { recursive: true });
    }
  } catch (e) {}
}

const tests = {};

function runTest(name, fn) {
  if (argv.only) {
    if (name.includes(argv.only)) {
      // test(name, fn);
      tests[name] = fn;
    }
  } else {
    tests[name] = fn;
    // test(name, fn);
    // fn();
  }
}

runTest('js: entry only', async () => {
  write(
    normalizeFiles({
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
function App() {
  return <div>App<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
    `,
    }),
  );
  const { process } = await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
  write({
    '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
function App() {
  return <div>App Modified<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")).render(<App />);
    `,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  console.log(`new html`, thisResult.html);
  assert.equal(thisResult.html, '<div>App Modified</div>', 'Initial render 2');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, true, 'isReload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('js: anonymize default export hmr', async () => {
  write(
    normalizeFiles({
      '/src/App.tsx': `
      export default function() {
        return <div>App</div>;
      };
            `,
      '/src/index.tsx': `
      import React from 'react';
      import ReactDOM from "react-dom/client";
      import App from './App';
      ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
          `,
    }),
  );
  const { process } = await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
  write({
    '/src/App.tsx': `
export default function() {
  return <div>App Modified</div>;
};
    `,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  console.log(`new html`, thisResult.html);
  assert.equal(thisResult.html, '<div>App Modified</div>', 'Initial render 2');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, false, 'isReload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('js: anonymize default export hmr (arrow function)', async () => {
  write(
    normalizeFiles({
      '/src/App.tsx': `
      export default () => {
        return <div>App</div>;
      };
            `,
      '/src/index.tsx': `
      import React from 'react';
      import ReactDOM from "react-dom/client";
      import App from './App';
      ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
          `,
    }),
  );
  const { process } = await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
  write({
    '/src/App.tsx': `
export default () => {
  return <div>App Modified</div>;
};
    `,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  console.log(`new html`, thisResult.html);
  assert.equal(thisResult.html, '<div>App Modified</div>', 'Initial render 2');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, false, 'isReload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('css: entry > css', async () => {
  write(
    normalizeFiles({
      '/src/index.css': `.foo {color:red;}`,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import "./index.css";
function App() {
  return <div className="foo">App<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
    `,
    }),
  );
  await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  const lastColor = await getElementColor(page, '.foo');
  assert.equal(lastColor, 'rgb(255, 0, 0)', 'Initial render');
  write({
    '/src/index.css': `.foo {color:blue;}`,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  const thisColor = await getElementColor(page, '.foo');
  console.log(`new color`, thisColor);
  assert.equal(thisColor, 'rgb(0, 0, 255)', 'Second render');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, false, 'should not reload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('css: entry > css modules', async () => {
  write(
    normalizeFiles({
      '/src/index.module.css': `.foo {color:red;}`,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import styles from "./index.module.css";
function App() {
  return <div className={\`\${styles.foo} foo\`}>App<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
    `,
    }),
  );
  await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  const lastColor = await getElementColor(page, '.foo');
  assert.equal(lastColor, 'rgb(255, 0, 0)', 'Initial render');
  write({
    '/src/index.module.css': `.foo {color:blue;}`,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  const thisColor = await getElementColor(page, '.foo');
  assert.equal(thisColor, 'rgb(0, 0, 255)', 'Second render');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, true, 'should reload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('css: entry > react component > css modules', async () => {
  write(
    normalizeFiles({
      '/src/index.module.css': `.foo {color:red;}`,
      '/src/App.tsx': `
import styles from "./index.module.css";
function App() {
  return <div className={\`\${styles.foo} foo\`}>App</div>;
}
export default App;
      `,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import App from './App';
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
    }),
  );
  await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  const lastColor = await getElementColor(page, '.foo');
  assert.equal(lastColor, 'rgb(255, 0, 0)', 'Initial render');
  write({
    '/src/index.module.css': `.foo {color:blue;}`,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  const thisColor = await getElementColor(page, '.foo');
  assert.equal(thisColor, 'rgb(0, 0, 255)', 'Second render');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, false, 'should not reload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('css: entry > css hmr with hostname runtime public', async () => {
  write(
    normalizeFiles(
      {
        '/public/index.html': `
    <!DOCTYPE html>
    <html lang="en">
    <head>
      <meta charset="UTF-8">
      <meta http-equiv="X-UA-Compatible" content="IE=edge">
      <meta name="viewport" content="width=device-width, initial-scale=1.0">
      <title>Document</title>
      <script>
        // for test css hmr
        window.publicPath = 'http://localhost:3000/';
      </script>
    </head>
    <body>
      <div id="root"></div>
      <link rel="stylesheet" href="/index.css" />
      <script src="/index.js"></script>
    </body>
    </html>
          `,
        '/src/index.css': `.foo {color:red;}`,
        '/src/App.tsx': `
  import "./index.css";
  function App() {
    return <div className="foo">App</div>;
  }
  export default App;
        `,
        '/src/index.tsx': `
  import React from 'react';
  import ReactDOM from "react-dom/client";
  import App from './App';
  ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
      `,
      },
      { publicPath: 'runtime' },
    ),
  );
  await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  const lastColor = await getElementColor(page, '.foo');
  assert.equal(lastColor, 'rgb(255, 0, 0)', 'Initial render');
  write({
    '/src/index.css': `.foo {color:blue;}`,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  const thisColor = await getElementColor(page, '.foo');
  console.log(`new color`, thisColor, 'expect color', 'rgb(0, 0, 255)');
  assert.equal(thisColor, 'rgb(0, 0, 255)', 'Second render');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, false, 'should not reload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('css: entry > css chunk hmr with hashed chunk id', async () => {
  write(
    normalizeFiles(
      {
        '/public/index.html': `
    <!DOCTYPE html>
    <html lang="en">
    <head>
      <meta charset="UTF-8">
      <meta http-equiv="X-UA-Compatible" content="IE=edge">
      <meta name="viewport" content="width=device-width, initial-scale=1.0">
      <title>Document</title>
    </head>
    <body>
      <div id="root"></div>
      <script src="/index.js"></script>
    </body>
    </html>
          `,
        '/src/App.css': `.foo {color:red;}`,
        '/src/App.tsx': `
  import('./App.css');
  function App() {
    return <div className="foo">App</div>;
  }
  export default App;
        `,
        '/src/index.tsx': `
  import React from 'react';
  import ReactDOM from "react-dom/client";
  import App from './App';
  ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
      `,
      },
      { moduleIdStrategy: 'hashed' },
    ),
  );
  await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  const lastColor = await getElementColor(page, '.foo');
  assert.equal(lastColor, 'rgb(255, 0, 0)', 'Initial render');
  write({
    '/src/App.css': `.foo {color:blue;}`,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  const thisColor = await getElementColor(page, '.foo');
  console.log(`new color`, thisColor, 'expect color', 'rgb(0, 0, 255)');
  assert.equal(thisColor, 'rgb(0, 0, 255)', 'Second render');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, false, 'should not reload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('js: entry > js, remove then add back', async () => {
  write(
    normalizeFiles({
      '/src/util.ts': `
export function foo() {
  return 'foo';
}
      `,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo } from './util';
function App() {
  return <div>App {foo()}</div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
    }),
  );
  await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  assert.equal(lastResult.html, '<div>App foo</div>', 'Initial render');
  remove('/src/util.ts');
  await delay(DELAY_TIME);
  write({
    '/src/util.ts': `
export function foo() {
return 'bar';
}
    `,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  assert.equal(thisResult.html, '<div>App bar</div>', 'Second render');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, true, 'should reload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest(
  'js: entry > util > bar, remove util then add back, util and bar should work',
  async () => {
    write(
      normalizeFiles({
        '/src/bar.ts': `
export function bar() {
  return 'bar';
}
      `,
        '/src/util.ts': `
import { bar } from './bar';
export function foo() {
  return 'foo' + bar();
}
      `,
        '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo } from './util';
function App() {
  return <div>App {foo()}</div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
      }),
    );
    await startMakoDevServer();
    await delay(DELAY_TIME);
    const { browser, page } = await startBrowser();
    let lastResult;
    let thisResult;
    let isReload;
    lastResult = normalizeHtml(await getRootHtml(page));
    assert.equal(lastResult.html, '<div>App foobar</div>', 'Initial render');
    remove('/src/util.ts');
    await delay(DELAY_TIME);
    write({
      '/src/util.ts': `
import { bar } from './bar';
export function foo() {
return 'bar'+bar();
}
    `,
    });
    await delay(DELAY_TIME);
    thisResult = normalizeHtml(await getRootHtml(page));
    assert.equal(thisResult.html, '<div>App barbar</div>', 'Second render');
    isReload = lastResult.random !== thisResult.random;
    assert.equal(isReload, true, 'should reload');
    lastResult = thisResult;
    await cleanup({ process, browser });
  },
);

// TODO: fix
skip('js: entry > js, rename .ts to .tsx', async () => {
  write(
    normalizeFiles({
      '/src/util.ts': `
export function foo() {
  return 'foo';
}
      `,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo } from './util';
function App() {
  return <div>App {foo()}</div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
    }),
  );
  await startMakoDevServer();
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  assert.equal(lastResult.html, '<div>App foo</div>', 'Initial render');
  remove('/src/util.ts');
  await delay(DELAY_TIME);
  write({
    '/src/util.tsx': `
export function foo() {
return 'bar';
}
    `,
  });
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  assert.equal(thisResult.html, '<div>App bar</div>', 'Second render');
  isReload = lastResult.random !== thisResult.random;
  assert.equal(isReload, true, 'should reload');
  lastResult = thisResult;
  await cleanup({ process, browser });
});

runTest('js: entry > js', async () => {
  await commonTest(
    {
      '/src/util.ts': `
export function foo() {
  return 'foo';
}
`,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo } from './util';
function App() {
return <div>App {foo()}</div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
  `,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>App foo</div>', 'Initial render');
    },
    {
      '/src/util.ts': `
  export function foo() {
  return 'bar';
  }
      `,
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>App bar</div>', 'Second render');
    },
    true,
  );
});

runTest('js: entry > react component', async () => {
  await commonTest(
    {
      '/src/App.tsx': `
function App() {
  return <div>App</div>;
}
export default App;
      `,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import App from './App';
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
    },
    {
      '/src/App.tsx': `
function App() {
  return <div>App update</div>;
}
export default App;
`,
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>App update</div>', 'Second render');
    },
    false,
  );
});

runTest('js: entry > react component + js', async () => {
  await commonTest(
    {
      '/src/App.tsx': `
function App() {
  return <div>App</div>;
}
export function foo() {
  return 'foo';
}
export default App;
      `,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import App, { foo } from './App';
foo();
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
    },
    {
      '/src/App.tsx': `
export function foo() {
  return 'bar';
}
function App() {
  return <div>App update</div>;
}
export default App;
`,
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>App update</div>', 'Second render');
    },
    false,
  );
});

runTest('js: entry > react component > util, change util', async () => {
  await commonTest(
    {
      '/src/util.ts': `
export function foo() {
  return 'foo';
}
`,
      '/src/App.tsx': `
import { foo } from './util';
function App() {
  return <div>App {foo()}</div>;
}
export default App;
`,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import App from './App';
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>App foo</div>', 'Initial render');
    },
    {
      '/src/util.ts': `
export function foo() {
  return 'bar';
}
`,
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>App bar</div>', 'Second render');
    },
    false,
  );
});

runTest(
  'js: entry > react component > util, entry > foo > util, change util',
  async () => {
    await commonTest(
      {
        '/src/util.ts': `
export function foo() {
  return 'foo';
}
`,
        '/src/foo.ts': `
import { foo } from './util';
export function fooo() {
  return foo();
}
`,
        '/src/App.tsx': `
import { foo } from './util';
function App() {
  return <div>App {foo()}</div>;
}
export default App;
`,
        '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import App from './App';
import { fooo } from './foo';
fooo();
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
      },
      (lastResult) => {
        assert.equal(lastResult.html, '<div>App foo</div>', 'Initial render');
      },
      {
        '/src/util.ts': `
export function foo() {
  return 'bar';
}
`,
      },
      (thisResult) => {
        assert.equal(thisResult.html, '<div>App bar</div>', 'Second render');
      },
      true,
    );
  },
);

runTest(
  'js: entry > react component a, rename a to c, rename entry import a to c',
  async () => {
    await commonTest(
      {
        '/src/A.tsx': `
function A() {
  return <div>A</div>;
}
export default A;
`,
        '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import A from './A';
ReactDOM.createRoot(document.getElementById("root")!).render(<><A /><section>{Math.random()}</section></>);
`,
      },
      (lastResult) => {
        assert.equal(lastResult.html, '<div>A</div>', 'Initial render');
      },
      {
        '/src/A.tsx': `
function C() {
  return <div>C</div>;
}
export default C;
`,
        '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import C from './A';
ReactDOM.createRoot(document.getElementById("root")!).render(<><C /><section>{Math.random()}</section></>);
`,
      },
      (thisResult) => {
        assert.equal(thisResult.html, '<div>C</div>', 'Second render');
      },
      true,
    );
  },
);

skip('js: entry > react component a > util b, rename b to c, rename a import b to c', async () => {
  await commonTest(
    {
      '/src/util.ts': `
export function b() {
  return 'b';
}
`,
      '/src/A.tsx': `
import { b } from './util';
function A() {
  return <div>A {b()}</div>;
}
export default A;
`,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import A from './A';
ReactDOM.createRoot(document.getElementById("root")!).render(<><A /><section>{Math.random()}</section></>);
`,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>A b</div>', 'Initial render');
    },
    {
      '/src/util.ts': `
export function c() {
  return 'c';
}
`,
      '/src/A.tsx': `
import { c } from './util';
function A() {
  return <div>A {c()}</div>;
}
export default A;
`,
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>A c</div>', 'Second render');
    },
    false,
  );
});

skip('js: entry > react component a > util b, remove b then add back', async () => {
  await commonTest(
    {
      '/public/index.css': ``,
      '/src/util.ts': `
export function b() {
  return 'b';
}
`,
      '/src/A.tsx': `
import { b } from './util';
function A() {
  return <div>A {b()}</div>;
}
export default A;
`,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import A from './A';
ReactDOM.createRoot(document.getElementById("root")!).render(<><A /><section>{Math.random()}</section></>);
`,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>A b</div>', 'Initial render');
    },
    () => {
      remove('src/util.ts');
      write({
        '/src/util.ts': `
export function b() {
  return 'b2';
}
`,
      });
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>A b2</div>', 'Second render');
    },
    false,
  );
});

runTest('js: entry, change and change back', async () => {
  let lastRandom;
  await commonTest(
    {
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
function App() {
  return <div>App<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
`,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
      lastRandom = lastResult.random;
    },
    async ({ page }) => {
      write({
        '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
function App() {
  return <div>App update<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
`,
      });
      await delay(DELAY_TIME);
      const newResult = normalizeHtml(await getRootHtml(page));
      assert.equal(newResult.html, '<div>App update</div>', 'Second render');
      assert.notEqual(lastRandom, newResult.random, `should reload`);
      write({
        '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
function App() {
  return <div>App<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
`,
      });
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>App</div>', 'Third render');
    },
    true,
  );
});

runTest('js: entry > react component, change twice quickly', async () => {
  await commonTest(
    {
      '/src/App.tsx': `
function App() {
  return <div>App</div>;
}
export default App;
      `,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import App from './App';
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
    `,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
    },
    {
      '/src/App.tsx': `
function App() {
  return <div>App 2</div>;
}
export default App;
`,
      '/src/App.tsx': `
function App() {
  return <div>App 3</div>;
}
export default App;
`,
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>App 3</div>', 'Second render');
    },
    false,
  );
});

skip('js: entry > react component, git checkout 2 files modified', async () => {
  await commonTest(
    {
      '/src/App.tsx': `
function App() {
  return <div>App</div>;
}
export default App;
`,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import App from './App';
ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
`,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
    },
    async () => {
      const gitPath = path.join(tmp, '.git');
      if (fs.existsSync(gitPath)) {
        await $`rm -rf ${gitPath}`;
      }
      await $`cd ${tmp} && git init && git checkout -b master && git add src && git commit -m "add" && git checkout -b newbranch`;
      write({
        '/src/App.tsx': `
function App() {
  return <div>App2</div>;
}
export default App;
`,
        '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import App2 from './App';
ReactDOM.createRoot(document.getElementById("root")!).render(<><App2 /><section>{Math.random()}</section></>);
`,
      });
      await $`cd ${tmp} && git add src && git commit -m "add" && git checkout master`;
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>App</div>', 'Second render');
    },
    true,
  );
});

skip('js: entry > react component a > util b, git checkout a&b modified', async () => {
  await $`rm -rf ${tmp}`;
  await commonTest(
    {
      '/src/util.ts': `
export function b() {
  return 'b';
}
`,
      '/src/A.tsx': `
import { b } from './util';
function A() {
  return <div>A {b()}</div>;
}
export default A;
`,
      '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import A from './A';
ReactDOM.createRoot(document.getElementById("root")!).render(<><A /><section>{Math.random()}</section></>);
`,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>A b</div>', 'Initial render');
    },
    async () => {
      const gitPath = path.join(tmp, '.git');
      if (fs.existsSync(gitPath)) {
        await $`rm -rf ${gitPath}`;
      }
      await $`cd ${tmp} && git init && git checkout -b master && git add src && git commit -m "add" && git checkout -b newbranch`;
      write({
        '/src/util.ts': `
export function c() {
  return 'c';
}
`,
        '/src/A.tsx': `
import { c } from './util';
function A() {
  return <div>A {c()}</div>;
}
export default A;
`,
      });
      await $`cd ${tmp} && git add src && git commit -m "add" && git checkout master`;
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>A b</div>', 'Second render');
    },
    false,
  );
});

runTest('add missing dep after watch start', async () => {
  await commonTest(
    {
      '/src/index.tsx': `
        import React from 'react';
        import ReactDOM from "react-dom/client";
        import App from './App';
        ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
      `,
    },
    (lastResult) => {
      assert.equal(lastResult.html, '', 'Initial render');
    },
    {
      '/src/App.tsx': `
        function App() {
          return <div>App</div>;
        }
        export default App;
      `,
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>App</div>', 'Second render');
    },
    true,
  );
});

runTest('issue: 861', async () => {
  write(
    normalizeFiles({
      '/src/index.tsx': `
        import React from 'react';
        import ReactDOM from "react-dom/client";
        import App from './App';
        ReactDOM.createRoot(document.getElementById("root")!).render(<><App /><section>{Math.random()}</section></>);
      `,
      '/src/App.tsx': `
        // import './foo';
        function App() {
          return <div>App</div>;
        }
        export default App;
      `,
      '/src/foo.ts': `
        console.log\('foo/foo');
      `,
    }),
  );
  await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  lastResult = normalizeHtml(await getRootHtml(page));
  assert.equal(lastResult.html, '<div>App</div>', 'Initial render');
  write({
    '/src/App.tsx': `
      import './foo';
      function App() {
        return <div>App</div>;
      }
      export default App;
    `,
  });
  await delay(DELAY_TIME);
  write({
    '/src/foo.ts': `
      console.log('foo/foo');
    `,
  });
  await delay(DELAY_TIME);
  thisResult = lastResult;
  lastResult = normalizeHtml(await getRootHtml(page));
  assert.equal(lastResult.html, '<div>App</div>', 'Second render');
  await cleanup({ process, browser });
});

runTest('link npm packages: modify file', async () => {
  await commonTest(
    async () => {
      write(
        normalizeFiles({
          '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo } from "mako-test-package-link";
function App() {
  return <div>{foo}<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
    `,
        }),
      );
      writePackage(
        'mako-test-package-link',
        normalizeFiles({
          'package.json': `
{
"name": "mako-test-package-link",
"version": "1.0.0",
"main": "index.js"
}
  `,
          'index.js': `
export * from './src/index';
  `,
          'src/index.js': `
const foo = 'foo';
export { foo };
  `,
        }),
      );
      execSync(
        'cd ./tmp/packages/mako-test-package-link && pnpm link --global',
      );
      execSync('pnpm link --global mako-test-package-link');
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>foo</div>', 'Initial render');
    },
    async () => {
      writePackage(
        'mako-test-package-link',
        normalizeFiles({
          'src/index.js': `
      const foo = 'foo2';
      export { foo };
    `,
        }),
      );
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>foo2</div>', 'Second render');
    },
    true,
  );
});

runTest('link npm packages: add file and import it', async () => {
  await commonTest(
    async () => {
      write(
        normalizeFiles({
          '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo } from "mako-test-package-link";
function App() {
  return <div>{foo}<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
    `,
        }),
      );
      writePackage(
        'mako-test-package-link',
        normalizeFiles({
          'package.json': `
{
"name": "mako-test-package-link",
"version": "1.0.0",
"main": "index.js"
}
  `,
          'index.js': `
export * from './src/index';
  `,
          'src/index.js': `
const foo = 'foo';
export { foo };
  `,
        }),
      );
      execSync(
        'cd ./tmp/packages/mako-test-package-link && pnpm link --global',
      );
      execSync('pnpm link --global mako-test-package-link');
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>foo</div>', 'Initial render');
    },
    async () => {
      // add files
      writePackage(
        'mako-test-package-link',
        normalizeFiles({
          'src/index2.js': `
        const bar = 'bar';
        export { bar };
      `,
          'index.js': `
    export * from './src/index';
    export * from './src/index2';
      `,
        }),
      );
      await delay(DELAY_TIME);
      // add import to added file
      write(
        normalizeFiles({
          '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo, bar } from "mako-test-package-link";
function App() {
  return <div>{foo}{bar}<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
    `,
        }),
      );
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>foobar</div>', 'Second render');
    },
    true,
  );
});

runTest('link npm packages: import a not exit file then add it', async () => {
  await commonTest(
    async () => {
      write(
        normalizeFiles({
          '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo } from "mako-test-package-link";
function App() {
  return <div>{foo}<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
    `,
        }),
      );
      writePackage(
        'mako-test-package-link',
        normalizeFiles({
          'package.json': `
{
"name": "mako-test-package-link",
"version": "1.0.0",
"main": "index.js"
}
  `,
          'index.js': `
export * from './src/index';
  `,
          'src/index.js': `
const foo = 'foo';
export { foo };
  `,
        }),
      );
      execSync(
        'cd ./tmp/packages/mako-test-package-link && pnpm link --global',
      );
      execSync('pnpm link --global mako-test-package-link');
    },
    (lastResult) => {
      assert.equal(lastResult.html, '<div>foo</div>', 'Initial render');
    },
    async () => {
      // add import to added file
      write(
        normalizeFiles({
          '/src/index.tsx': `
import React from 'react';
import ReactDOM from "react-dom/client";
import { foo, bar } from "mako-test-package-link";
function App() {
  return <div>{foo}{bar}<section>{Math.random()}</section></div>;
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
    `,
        }),
      );
      await delay(DELAY_TIME);
      // add files
      writePackage(
        'mako-test-package-link',
        normalizeFiles({
          'src/index2.js': `
        const bar = 'bar';
        export { bar };
      `,
          'index.js': `
    export * from './src/index';
    export * from './src/index2';
      `,
        }),
      );
    },
    (thisResult) => {
      assert.equal(thisResult.html, '<div>foobar</div>', 'Second render');
    },
    true,
  );
});

function normalizeFiles(files, makoConfig = {}) {
  return {
    '/public/index.html': `
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="X-UA-Compatible" content="IE=edge">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Document</title>
</head>
<body>
  <div id="root"></div>
  <link rel="stylesheet" href="/index.css" />
  <script src="/index.js"></script>
</body>
</html>
      `,
    '/mako.config.json':
      JSON.stringify(
        {
          ...makoConfig,
          minify: false,
        },
        null,
        2,
      ) + '\n',
    ...files,
  };
}

function write(files) {
  for (const [file, content] of Object.entries(files)) {
    const p = path.join(tmp, file);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, content, 'utf-8');
  }
}

function writePackage(packageName, files) {
  for (const [file, content] of Object.entries(files)) {
    const p = path.join(tmpPackages, packageName, file);
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, content, 'utf-8');
  }
}

function remove(file) {
  const p = path.join(tmp, file);
  fs.unlinkSync(p);
}

async function startMakoDevServer() {
  const p = $`${path.join(
    root,
    'scripts',
    'mako.js',
  )} ${tmp} --watch`.nothrow();
  await waitPort({ port: 3000, timeout: 10000 });
  return { process: p };
}

async function startBrowser() {
  const browser = await chromium.launch();
  const context = await browser.newContext(devices['iPhone 11']);
  const page = await context.newPage();
  await page.goto(`http://localhost:${port}`);
  return { browser, page };
}

async function getRootHtml(page) {
  const el = await page.$('#root');
  const html = await el.innerHTML();
  return html;
}

async function getElementColor(page, selector) {
  const el = await page.$(selector);
  const color = await el.evaluate((el) => {
    return window.getComputedStyle(el).color;
  });
  return color;
}

async function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function killMakoDevServer() {
  const res = await $`ps -ax | grep mako | grep -v grep | awk '{print $1}'`;
  console.error('stdout', res.stdout);
  await $`ps -ax | grep mako | grep -v grep | awk '{print $1}' | xargs kill -9`;
}

function normalizeHtml(html) {
  // e.g. <div>App<section>0.7551619733386135</section></div>
  const re = /<section>(.+?)<\/section>/;
  const match = html.match(re);
  const random = match?.[1];
  html = html.replace(re, '');
  return { html, random };
}

async function commonTest(
  initFilesOrFunc = {},
  lastResultCallback = () => {},
  modifyFilesOrCallback = () => {},
  thisResultCallback = () => {},
  shouldReload = false,
) {
  typeof initFilesOrFunc === 'function'
    ? await initFilesOrFunc()
    : write(normalizeFiles(initFilesOrFunc));
  await startMakoDevServer();
  await delay(DELAY_TIME);
  const { browser, page } = await startBrowser();
  let lastResult;
  let thisResult;
  let isReload;
  lastResult = normalizeHtml(await getRootHtml(page));
  lastResultCallback(lastResult);
  typeof modifyFilesOrCallback === 'function'
    ? await modifyFilesOrCallback({ page })
    : write(modifyFilesOrCallback);
  await delay(DELAY_TIME);
  thisResult = normalizeHtml(await getRootHtml(page));
  thisResultCallback(thisResult);
  isReload = lastResult.random !== thisResult.random;
  assert.equal(
    isReload,
    shouldReload,
    `should ${shouldReload ? '' : 'not '}reload`,
  );
  lastResult = thisResult;
  await cleanup({ process, browser });
}

(async () => {
  console.log('tests', Object.keys(tests).join(', '));
  for (const [name, fn] of Object.entries(tests)) {
    console.log(`> ${chalk.green(name)}`);
    await fn();
  }
})().catch(async (e) => {
  console.error(chalk.red(e));
  await cleanup();
  process.exit(1);
});

process.on('exit', async () => {
  await cleanup();
});
