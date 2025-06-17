import { createHotReloader } from "./hotReloader";
import { ProjectOptions } from "./types";
import { blockStdout } from "./util";
import { xcodeProfilingReady } from "./xcodeProfile";

export async function watch(
  options: ProjectOptions,
  projectPath: string,
  rootPath?: string,
) {
  blockStdout();

  if (process.env.XCODE_PROFILE) {
    await xcodeProfilingReady();
  }

  const hotReloader = await createHotReloader(options, projectPath, rootPath);
  await hotReloader.start();
}
