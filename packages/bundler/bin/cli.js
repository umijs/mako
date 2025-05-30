#!/usr/bin/env node
const { build } = require("../cjs/index.js");

console.log(process.argv);

// Get the directory argument, default to current directory
const dir = process.argv[2] || process.cwd();

build(dir);
