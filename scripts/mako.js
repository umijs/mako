#!/usr/bin/env node

const path = require('path');
const fs = require('fs');
const { build } = require('@okamjs/okam');
const { _lessLoader } = require('@alipay/umi-bundler-okam');
const cwd = process.argv[2];
const isWatch = process.argv.includes('--watch');

let makoConfig = {};
const makoConfigPath = path.join(cwd, 'mako.config.json');
if (fs.existsSync(makoConfigPath)) {
  makoConfig = JSON.parse(fs.readFileSync(makoConfigPath, 'utf-8'));
}
const alias = {};
if (makoConfig.resolve?.alias) {
  Object.keys(makoConfig.resolve.alias).forEach((key) => {
    alias[key] = path.join(cwd, makoConfig.resolve.alias[key]);
  });
}
const okamConfig = {
  resolve: { alias },
};
console.log('> run mako build for', cwd);
let hooks = {};
const hooksPath = path.join(cwd, 'hooks.config.js');
if (fs.existsSync(hooksPath)) {
  hooks = require(hooksPath);
}
build({
  root: cwd,
  config: okamConfig,
  hooks: {
    ...hooks,
    load: _lessLoader(null, {
      cwd,
      alias,
      modifyVars: makoConfig.less?.theme || {},
      config: {},
    }),
  },
  watch: isWatch,
}).catch((e) => {
  console.error(e);
  process.exit(1);
});
