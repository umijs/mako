const path = require('path');
const fs = require('fs');
const { chromium, devices } = require('playwright');
const express = require('express');
const { getPortPromise } = require('portfinder');

function parseBuildResult(cwd) {
  const distDir = path.join(cwd, 'dist');
  const files = fs.readdirSync(distDir).reduce((acc, file) => {
    acc[file] = fs.readFileSync(path.join(distDir, file), 'utf-8');
    return acc;
  }, {});
  return {
    distDir,
    files,
  };
}

async function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function testWithBrowser({
  cwd,
  fn,
  rootElement = 'root',
  entry = 'umi.js',
}) {
  const distDir = path.join(cwd, 'dist');
  const htmlPath = path.join(distDir, 'index.html');
  if (!fs.existsSync(htmlPath)) {
    const html = `
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Document</title>
</head>
<body>
  <div id="${rootElement}"></div>
  <script src="${entry}"></script>
</body>
</html>
    `.trimStart();
    fs.writeFileSync(htmlPath, html, 'utf-8');
  }
  const port = await getPortPromise();
  const server = await serve(distDir, port);
  const browser = await chromium.launch();
  const context = await browser.newContext({
    ...devices['iPhone 11 Pro'],
  });
  const page = await context.newPage();
  console.log(`http://localhost:${port}/`);
  await page.goto(`http://localhost:${port}/`);
  await fn({ page, browser, context });
  await browser.close();
  await closeServer(server);
}

async function serve(dir, port) {
  return new Promise((resolve, reject) => {
    const app = express();
    app.use(express.static(dir));
    const server = app.listen(port, () => {
      resolve(server);
    });
    setTimeout(() => {
      reject(new Error(`start server for ${dir} timeout`));
    }, 2000);
  });
}

async function closeServer(server) {
  return new Promise((resolve, reject) => {
    server.close((err) => {
      if (err) {
        reject(err);
      } else {
        resolve();
      }
    });
    setTimeout(() => {
      reject(new Error(`close server timeout`));
    }, 2000);
  });
}

const trim = (str) => {
  return str.replace(/[\s\n]/g, '');
};

/**
 * 自动转义字符串中的 (){}[] => \(\)\{\}\[\]，转正则表达式使用
 * @param {string} str
 * @returns
 */
const strEscape = (str) => {
  return str.replace(/(?<!\\)([\(\)\{\}\[\]])/g, '\\$1');
};

/**
 * string 转正则，自动转义 (){}[] => \(\)\{\}\[\]
 * @param {string} str
 * @returns
 */
const string2RegExp = (str) => {
  return new RegExp(strEscape(str));
};

const moduleReg = (key, contentReg, autoEscape) => {
  if (autoEscape) {
    contentReg = strEscape(contentReg);
  }
  return new RegExp(
    `"${key}": function\\(module, exports, __mako_require__\\) \\{[\\s\\S]*${contentReg}[\\s\\S]*\\}`,
  );
};

const injectSimpleJest = () => {
  function it(testName, fn) {
    try {
      fn();
      console.log('\x1B[34m\x1B[102mPASS\x1B[49m\x1B[39m', ':', testName);
    } catch (e) {
      throw e;
    }
  }

  function ignore(testName, fn) {
    // chalk.blueBright(chalk.bgYellowBright('IGNORED'))
    console.log('\x1B[94m\x1B[103mIGNORED\x1B[49m\x1B[39m', ':', testName);
  }

  global.it = it;
  global.it.skip = ignore;
  global.xit = ignore;

  global.expect = require('@jest/expect').jestExpect;
};

exports.parseBuildResult = parseBuildResult;
exports.trim = trim;
exports.string2RegExp = string2RegExp;
exports.moduleReg = moduleReg;
exports.testWithBrowser = testWithBrowser;
exports.injectSimpleJest = injectSimpleJest;
