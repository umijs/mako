import fs from "fs";
import path from "path";
import * as utooPack from "@utoo/pack";
import { Command, Flags } from "@oclif/core";

export default class Watch extends Command {
  static description = "Say hello";
  static examples = [
    `<%= config.bin %> <%= command.id %> dev --project .`,
    `<%= config.bin %> <%= command.id %> dev --project . --root ../..`,
  ];
  static flags = {
    project: Flags.string({
      char: "p",
      description: "Set the project path",
      required: false,
    }),
    root: Flags.string({
      char: "r",
      description: "Set the root path",
      required: false,
    }),
  };

  async run(): Promise<void> {
    const {
      flags: { project, root },
    } = await this.parse(Watch);

    const cwd = process.cwd();

    const projectOptions = JSON.parse(
      fs.readFileSync(
        path.resolve(cwd, project || "", "project_options.json"),
        {
          encoding: "utf-8",
        },
      ),
    );

    await utooPack.watch(
      projectOptions,
      path.resolve(cwd, project || cwd),
      path.resolve(cwd, root || project || cwd),
    );
  }
}
