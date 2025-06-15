import { nanoid } from "nanoid";
import { projectFactory } from "./project";
import fs from "fs";
import path from "path";
import { createDefineEnv, formatIssue, isRelevantWarning } from "./util";
import { ProjectOptions } from "./types";
import { xcodeProfilingReady } from "./xcodeProfile";

// ref:
// https://github.com/vercel/next.js/pull/51883
function blockStdout() {
  // rust needs stdout to be blocking, otherwise it will throw an error (on macOS at least) when writing a lot of data (logs) to it
  // see https://github.com/napi-rs/napi-rs/issues/1630
  // and https://github.com/nodejs/node/blob/main/doc/api/process.md#a-note-on-process-io
  if ((process.stdout as any)._handle != null) {
    (process.stdout as any)._handle.setBlocking(true);
  }
  if ((process.stderr as any)._handle != null) {
    (process.stderr as any)._handle.setBlocking(true);
  }
}

export async function build(dir?: string) {
  blockStdout();

  if (process.env.XCODE_PROFILE) {
    await xcodeProfilingReady();
  }

  const cwd = dir || process.cwd();

  const projectOptions: ProjectOptions = JSON.parse(
    fs.readFileSync(path.join(cwd, "project_options.json"), {
      encoding: "utf-8",
    }),
  );

  await buildWithOptions(projectOptions, cwd);
}

export async function buildWithOptions(options: ProjectOptions, cwd?: string) {
  const workingDir = cwd || process.cwd();

  const createProject = projectFactory();
  const project = await createProject(
    {
      processEnv: options.processEnv ?? ({} as Record<string, string>),
      processDefineEnv: options.processDefineEnv ?? createDefineEnv({
        config: options.config,
        dev: options.dev ?? false,
      }),
      watch: options.watch ?? {
        enable: false,
      },
      dev: options.dev ?? false,
      buildId: nanoid(),
      config: options.config,
      rootPath: path.resolve(workingDir, options.rootPath),
      projectPath: path.resolve(workingDir, options.projectPath),
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