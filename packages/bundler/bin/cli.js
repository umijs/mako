#!/usr/bin/env node
const { build } = require("../cjs/index.js");

build(process.argv[process.argv.length - 1]);
