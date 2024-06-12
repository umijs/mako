#!/usr/bin/env node

const path = require('path');
const fs = require('fs');
const { build } = require('@umijs/mako');
const cwd = process.argv[2];

console.log('> run mako build for', cwd);
const config = getMakoConfig();
build({
  root: cwd,
  config,
  watch: process.argv.includes('--watch'),
}).catch((e) => {
  console.error(e);
  process.exit(1);
});

function getPlugins() {
  let plugins = [];
  const hooksPath = path.join(cwd, 'hooks.config.js');
  if (fs.existsSync(hooksPath)) {
    let hooks = require(hooksPath);
    plugins.push(hooks);
  }
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
  makoConfig.resolve.alias = makoConfig.resolve.alias || {};
  makoConfig.less = {
    modifyVars: makoConfig.less?.theme || {},
  };
  makoConfig.plugins = getPlugins();
  Object.keys(makoConfig.resolve.alias).forEach((key) => {
    makoConfig.resolve.alias[key] = path.join(
      cwd,
      makoConfig.resolve.alias[key],
    );
  });
  return makoConfig;
}
