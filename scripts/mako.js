#!/usr/bin/env node

const path = require('path');
const fs = require('fs');
const { build } = require('@umijs/mako');
const cwd = process.argv[2];

console.log('> run mako build for', cwd);
const config = getMakoConfig();
const watch = process.argv.includes('--watch');
build({
  root: cwd,
  config,
  watch,
})
  .then(() => {
    if (!watch) {
      process.exit(0);
    }
  })
  .catch((e) => {
    console.error(e);
    process.exit(1);
  });

function getPlugins() {
  let plugins = [];
  const pluginsPath = path.join(cwd, 'plugins.config.js');
  if (fs.existsSync(pluginsPath)) {
    plugins.push(...require(pluginsPath));
  }
  return plugins;
}

function getMakoConfig() {
  let makoConfig = {};
  const makoConfigPath = path.join(cwd, 'mako.config.json');
  if (fs.existsSync(makoConfigPath)) {
    makoConfig = JSON.parse(fs.readFileSync(makoConfigPath, 'utf-8'));
  }
  makoConfig.resolve = makoConfig.resolve || {};
  makoConfig.resolve.alias = makoConfig.resolve.alias || [];
  makoConfig.less = {
    modifyVars: makoConfig.less?.theme || {},
    globalVars: makoConfig.less?.globalVars,
  };
  makoConfig.plugins = getPlugins();
  makoConfig.resolve.alias.forEach((alias) => {
    alias[1] = path.join(cwd, alias[1]);
  });
  return makoConfig;
}
