const assert = require("assert");
const { testWithBrowser } = require("../../../scripts/test-utils");

const test = async () => {
  try {
    await testWithBrowser({
      cwd: __dirname,
      fn: async ({ page }) => {
        const elm = await page.locator('#root');
        const content = await elm.evaluate((el) => el.innerHTML);
        assert.equal(content, 'a utils', 'async chunk and common chunk should be loaded');

        const styles = await elm.evaluate((el) => window.getComputedStyle(el));
        assert.equal(styles.fontSize, '100px', 'async css chunk should be loaded');
      },
      entry: "index.js",
    });
  } catch (e) {
    throw new Error(e);
  }
};
module.exports = test;
