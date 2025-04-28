import { nanoid } from "nanoid";
import { projectFactory } from "./project";
import fs from "fs";
import path from "path";
import { formatIssue, isRelevantWarning, processIssues } from "./util";
import { ProjectOptions } from "./types";

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

  const cwd = dir || process.cwd();

  const projectOptions: ProjectOptions = JSON.parse(
    fs.readFileSync(path.join(cwd, "project_options.json"), {
      encoding: "utf-8",
    }),
  );

  const createProject = projectFactory();
  const project = await createProject(
    {
      processEnv: projectOptions.processEnv ?? ({} as Record<string, string>),
      processDefineEnv: projectOptions.processDefineEnv ?? {
        client: [],
        nodejs: [],
        edge: [],
      },
      watch: projectOptions.watch ?? {
        enable: false,
      },
      dev: projectOptions.dev ?? false,
      buildId: nanoid(),
      config: projectOptions.config,
      rootPath: path.resolve(cwd, projectOptions.rootPath),
      projectPath: path.resolve(cwd, projectOptions.projectPath),
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

  // TODO: Need to exit manually now. May run tasks in worker is a better way, see
  // https://github.com/vercel/next.js/blob/512d8283054407ab92b2583ecce3b253c3be7b85/packages/next/src/lib/worker.ts

  if (!process.env.BUNDLER_TURBOPACK_TRACE_SERVER) {
    process.exit();
  }
}
