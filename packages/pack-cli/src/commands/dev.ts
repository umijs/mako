import fs from "fs";
import path from "path";
import * as utooPack from "@utoo/pack";
import { Command, Flags } from "@oclif/core";

export default class Dev extends Command {
  static description = "Utoo pack watch";
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
    } = await this.parse(Dev);

    const cwd = process.cwd();

    const projectOptions = JSON.parse(
      fs.readFileSync(
        path.resolve(cwd, project || "", "project_options.json"),
        {
          encoding: "utf-8",
        },
      ),
    );

    await utooPack.serve(
      projectOptions,
      path.resolve(cwd, project || cwd),
      path.resolve(cwd, root || project || cwd),
    );
  }
}
