import { nanoid } from "nanoid";
import { projectFactory } from "./project";
import path from "path";
import {
  blockStdout,
  createDefineEnv,
  formatIssue,
  isRelevantWarning,
} from "./util";
import { ProjectOptions } from "./types";
import { xcodeProfilingReady } from "./xcodeProfile";

export async function build(
  projectOptions: Omit<ProjectOptions, "projectPath" | "rootPath">,
  projectPath?: string,
  rootPath?: string,
) {
  blockStdout();

  if (process.env.XCODE_PROFILE) {
    await xcodeProfilingReady();
  }

  const createProject = projectFactory();
  const project = await createProject(
    {
      processEnv: projectOptions.processEnv ?? ({} as Record<string, string>),
      processDefineEnv: createDefineEnv({
        config: projectOptions.config,
        dev: projectOptions.dev ?? false,
        optionDefineEnv: projectOptions.processDefineEnv,
      }),
      watch: projectOptions.watch ?? {
        enable: false,
      },
      dev: projectOptions.dev ?? false,
      buildId: nanoid(),
      config: projectOptions.config,
      projectPath: projectPath || process.cwd(),
      rootPath: rootPath || projectPath || process.cwd(),
    },
    {
      persistentCaching: false,
    },
  );

  const entrypoints = await project.writeAllEntrypointsToDisk();

  const topLevelErrors = [];
  const topLevelWarnings = [];
  for (const issue of entrypoints.issues) {
    if (issue.severity === "error" || issue.severity === "fatal") {
      topLevelErrors.push(formatIssue(issue));
    } else if (isRelevantWarning(issue)) {
      topLevelWarnings.push(formatIssue(issue));
    }
  }

  if (topLevelWarnings.length > 0) {
    console.warn(
      `Turbopack build encountered ${
        topLevelWarnings.length
      } warnings:\n${topLevelWarnings.join("\n")}`,
    );
  }

  if (topLevelErrors.length > 0) {
    throw new Error(
      `Turbopack build failed with ${
        topLevelErrors.length
      } errors:\n${topLevelErrors.join("\n")}`,
    );
  }

  await project.shutdown();

  // TODO: Maybe run tasks in worker is a better way, see
  // https://github.com/vercel/next.js/blob/512d8283054407ab92b2583ecce3b253c3be7b85/packages/next/src/lib/worker.ts
}
