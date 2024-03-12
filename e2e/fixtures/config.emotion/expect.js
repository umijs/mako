#!/usr/bin/env zx

const assert = require("assert");
const { testWithBrowser, string2RegExp } = require("../../../scripts/test-utils");

const test = async () => {
  try {
    await testWithBrowser({
      cwd: __dirname,
      fn: async ({ page }) => {
        // css`` and css property
        const element1 = await page.locator("id=first");
        const color1 = await element1.evaluate((el) => {
          return window.getComputedStyle(el).getPropertyValue("color");
        });
        assert(
          string2RegExp("rgb(1, 1, 1)").test(color1),
          "css`` and css property should work"
        );

        // styled.div({})
        const element2 = await page.locator("id=second");
        const background1 = await element2.evaluate((el) => {
          return window.getComputedStyle(el).getPropertyValue("background");
        });
        assert(
          string2RegExp("rgb(2, 2, 2)").test(background1),
          "styled.div({}) should work"
        );

        // styled.dev``
        const element3 = await page.locator("id=third");
        const color2 = await element3.evaluate((el) => {
          return window.getComputedStyle(el).getPropertyValue("color");
        });
        assert(
          string2RegExp("rgb(3, 3, 3)").test(color2),
          "styled.dev`` should work"
        );

        // nested style.div``
        const element4 = await page.locator("id=forth");
        const color3 = await element4.evaluate((el) => {
          return window.getComputedStyle(el).getPropertyValue("color");
        });
        assert(
          string2RegExp("rgb(4, 4, 4)").test(color3),
          "nested style.div`` should work"
        );
      },
      entry: "index.js",
    });
  } catch (e) {
    throw new Error(e);
  }
};
module.exports = test;
