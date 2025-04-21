import { nanoid } from "nanoid";
import { projectFactory } from "./project";
import fs from "fs";
import path from "path";
import { formatIssue } from "./util";
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
      entry: projectOptions.entry,
      distDir: projectOptions.distDir,
      env: projectOptions.env ?? ({} as Record<string, string>),
      defineEnv: projectOptions.defineEnv ?? {
        client: [],
        nodejs: [],
        edge: [],
      },
      jsConfig: projectOptions.jsConfig ?? {
        compilerOptions: {},
      },
      watch: projectOptions.watch ?? {
        enable: false,
      },
      dev: false,
      buildId: nanoid(),
      noMangling: projectOptions.noMangling ?? false,
      browserslistQuery:
        "last 1 Chrome versions, last 1 Firefox versions, last 1 Safari versions, last 1 Edge versions",
      // FIXME
      config: {
        env: projectOptions.config?.env ?? {},
        experimental: projectOptions.config?.experimental ?? {},
        lessOptions: projectOptions.config?.lessOptions,
        sassOptions: projectOptions.config?.sassOptions,
      },
      rootPath: path.resolve(cwd, projectOptions.rootPath),
      projectPath: path.resolve(cwd, projectOptions.projectPath),
    },
    {
      persistentCaching: false,
    },
  );
  const entrypointsSubscription = project.entrypointsSubscribe();
  const entrypointsResult = await entrypointsSubscription.next();
  if (entrypointsResult.done) {
    throw new Error("Turbopack did not return any entrypoints");
  }
  entrypointsSubscription.return?.().catch(() => {});

  const entrypoints = entrypointsResult.value;

  const topLevelErrors: {
    message: string;
  }[] = [];

  for (const issue of entrypoints.issues) {
    topLevelErrors.push({
      message: formatIssue(issue),
    });
  }

  if (topLevelErrors.length > 0) {
    throw new Error(
      `Turbopack build failed with ${
        topLevelErrors.length
      } issues:\n${topLevelErrors.map((e) => e.message).join("\n")}`,
    );
  }

  await Promise.all(
    entrypointsResult.value.libraries.map((l) => l.writeToDisk()),
  );

  await project.shutdown();

  console.log(`${new Date().toISOString()} ****** finished ******`);

  // TODO: Need to exit manually now. May run tasks in worker is a better way, see
  // https://github.com/vercel/next.js/blob/512d8283054407ab92b2583ecce3b253c3be7b85/packages/next/src/lib/worker.ts

  if (!process.env.BUNDLER_TURBOPACK_TRACE_SERVER) {
    process.exit();
  }
}
