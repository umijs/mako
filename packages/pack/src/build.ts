import { nanoid } from "nanoid";
import { projectFactory } from "./project";
import { BundleOptions } from "./types";
import {
  blockStdout,
  createDefineEnv,
  formatIssue,
  isRelevantWarning,
} from "./util";
import { compatOptionsFromWebpack, WebpackConfig } from "./webpackCompat";
import { xcodeProfilingReady } from "./xcodeProfile";

export function build(
  bundleOptions: BundleOptions,
  projectPath?: string,
  rootPath?: string,
): Promise<void>;

export function build(
  webpackConfig: WebpackConfig,
  projectPath?: string,
  rootPath?: string,
): Promise<void>;

export function build(
  options: BundleOptions | WebpackConfig,
  projectPath?: string,
  rootPath?: string,
) {
  const bundleOptions = (<WebpackConfig>options).compatMode
    ? compatOptionsFromWebpack(<WebpackConfig>options)
    : <BundleOptions>options;
  return buildInternal(bundleOptions, projectPath, rootPath);
}

async function buildInternal(
  bundleOptions: BundleOptions,
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
      processEnv: bundleOptions.processEnv ?? {},
      processDefineEnv: createDefineEnv({
        config: bundleOptions.config,
        dev: bundleOptions.dev ?? false,
        optionDefineEnv: bundleOptions.processDefineEnv,
      }),
      watch: {
        enable: false,
      },
      dev: bundleOptions.dev ?? false,
      buildId: bundleOptions.buildId || nanoid(),
      config: bundleOptions.config,
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
