#!/usr/bin/env node
const path = require("path");
const fs = require("fs");
const { Command } = require("commander");
const bundler = require("../cjs/index.js");

const program = new Command();
program
  .name("utoo-pack")
  .version(require(path.join(__dirname, "../package.json")).version);

program
  .argument("<mode>", "build or watch")
  .option("-p, --project <string>", "project dir")
  .option("-r, --root <string>", "root dir")
  .action((mode, { project, root }) => {
    const cwd = process.cwd();
    const projectOptions = JSON.parse(
      fs.readFileSync(path.join(project, "project_options.json"), {
        encoding: "utf-8",
      }),
    );
    bundler[mode](
      projectOptions,
      path.resolve(cwd, project),
      path.resolve(cwd, root || project),
      cwd,
    );
  });

program.parse(process.argv);
