import { nanoid } from "nanoid";
import { projectFactory } from "./project";
import fs from "fs";
import path from "path";

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

  const projectOptions = JSON.parse(
    fs.readFileSync(path.join(cwd, "project_options.json"), {
      encoding: "utf-8",
    }),
  );

  const createProject = projectFactory();
  const project = await createProject(
    {
      env: {} as Record<string, string>,
      defineEnv: { client: [], nodejs: [], edge: [] },
      config: { env: {}, experimental: {} },
      jsConfig: {
        compilerOptions: {},
      },
      watch: {
        enable: false,
      },
      dev: false,
      buildId: nanoid(),
      noMangling: false,
      browserslistQuery:
        "last 1 Chrome versions, last 1 Firefox versions, last 1 Safari versions, last 1 Edge versions",
      ...projectOptions,
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
  await Promise.all(
    entrypointsResult.value.libraries.map((l) => l.writeToDisk()),
  );
  entrypointsSubscription.return?.().catch(() => {});
}
