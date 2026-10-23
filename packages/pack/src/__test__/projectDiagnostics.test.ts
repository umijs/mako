import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { NapiIssue, NapiWrittenEndpoint } from "../binding";
import type { Endpoint, Project, ProjectOptions } from "../core/types";

const bindingMocks = vi.hoisted(() => ({
  endpointClientChangedSubscribe: vi.fn(),
  endpointServerChangedSubscribe: vi.fn(),
  endpointWriteToDisk: vi.fn(),
  projectEntrypointsSubscribe: vi.fn(),
  projectHmrEvents: vi.fn(),
  projectHmrIdentifiersSubscribe: vi.fn(),
  projectNew: vi.fn(),
  projectOnExit: vi.fn(),
  projectShutdown: vi.fn(),
  projectUpdateInfoSubscribe: vi.fn(),
  projectWriteAllEntrypointsToDisk: vi.fn(),
  registerWorkerScheduler: undefined,
  rootTaskDispose: vi.fn(),
}));

vi.mock("../binding", () => bindingMocks);

import { DiagnosticSources } from "../core/diagnosticSources";
import { projectFactory } from "../core/project";

type Callback = (error: Error | undefined, value: unknown) => void;
const nativeEndpoint = { __napiType: "Endpoint" as const };
const rootTask = { __napiType: "RootTask" as const };

function diagnostic(content = "export const value = 'before';") {
  const issue: NapiIssue = {
    severity: "warning",
    stage: "resolve",
    filePath: "[project]/source.js",
    title: { type: "text", value: "Cannot resolve dynamic import" },
    description: { type: "text", value: "Original description" },
    detail: { type: "text", value: "Original detail" },
    documentationLink: "https://example.test/diagnostic",
    importTraces: [{ trace: "source.js" }],
    source: {
      source: { ident: "[project]/source.js", content },
      range: {
        start: { line: 2, column: 3 },
        end: { line: 2, column: 9 },
      },
    },
  };
  return { issues: [issue] };
}

function writtenEndpoint(content?: string) {
  return {
    type: "nodejs",
    entryPath: "server/entry.js",
    clientPaths: ["static/entry.js"],
    serverPaths: [{ path: "server/entry.js", contentHash: "original-hash" }],
    config: {},
    ...diagnostic(content),
  } satisfies TurbopackResult<NapiWrittenEndpoint>;
}

function nativeEntrypoints(content?: string) {
  return {
    apps: [nativeEndpoint],
    libraries: [nativeEndpoint],
    appPaths: [writtenEndpoint(content)],
    libraryPaths: [writtenEndpoint(content)],
    ...diagnostic(content),
  };
}

const options: Required<ProjectOptions> = {
  rootPath: "/project",
  projectPath: "/project",
  packPath: "/pack",
  config: { entry: [], output: {} },
  dev: false,
  watch: { enable: false },
  processEnv: {},
  buildId: "production-diagnostics-test",
  tracing: false,
};

async function createProject() {
  return projectFactory()(options, {});
}

beforeEach(() => {
  vi.resetAllMocks();
  bindingMocks.projectNew.mockResolvedValue({ __napiType: "Project" });
  bindingMocks.projectShutdown.mockResolvedValue(undefined);
  bindingMocks.projectOnExit.mockResolvedValue(undefined);
  bindingMocks.projectWriteAllEntrypointsToDisk.mockImplementation(async () =>
    nativeEntrypoints(),
  );
  bindingMocks.endpointWriteToDisk.mockImplementation(async () =>
    writtenEndpoint(),
  );
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("project diagnostic compatibility", () => {
  it("preserves production write results and shares their project cache with app and library endpoints", async () => {
    const intern = vi.spyOn(DiagnosticSources.prototype, "intern");
    const native = nativeEntrypoints();
    const expected = structuredClone(native);
    bindingMocks.projectWriteAllEntrypointsToDisk.mockResolvedValue(native);
    const project = await createProject();
    const result = await project.writeAllEntrypointsToDisk();
    const projectCache = intern.mock.contexts[0];

    expect(result.issues).toEqual(expected.issues);
    expect(result.appPaths).toEqual(expected.appPaths);
    expect(result.libraryPaths).toEqual(expected.libraryPaths);
    expect(native).toEqual(expected);

    for (const endpoint of [...result.apps!, ...result.libraries!]) {
      const written = writtenEndpoint();
      const writtenBefore = structuredClone(written);
      bindingMocks.endpointWriteToDisk.mockResolvedValueOnce(written);
      const response = await endpoint.writeToDisk();

      expect(response).toBe(written);
      expect(response).toEqual(writtenBefore);
      expect(intern.mock.contexts.at(-1)).toBe(projectCache);
    }

    const edited = writtenEndpoint("export const value = 'after';");
    bindingMocks.endpointWriteToDisk.mockResolvedValueOnce(edited);
    expect(await result.apps![0].writeToDisk()).toEqual(edited);
    expect(result.issues[0].source?.source.content).toBe(
      "export const value = 'before';",
    );
    expect(bindingMocks.projectNew.mock.calls[0][0]).toMatchObject({
      dev: false,
      watch: { enable: false },
    });
    expect(bindingMocks.projectNew.mock.calls[0][1]).toEqual({});
    expect(bindingMocks.projectHmrEvents).not.toHaveBeenCalled();
    expect(bindingMocks.projectEntrypointsSubscribe).not.toHaveBeenCalled();
  });

  it("isolates projects created by the same factory and clears each cache on shutdown or exit", async () => {
    const intern = vi.spyOn(DiagnosticSources.prototype, "intern");
    const clear = vi.spyOn(DiagnosticSources.prototype, "clear");
    const factory = projectFactory();
    const first = await factory(options, {});
    const second = await factory(options, {});
    await first.writeAllEntrypointsToDisk();
    const firstCache = intern.mock.contexts[0];
    intern.mockClear();
    await second.writeAllEntrypointsToDisk();
    const secondCache = intern.mock.contexts[0];

    expect(firstCache).not.toBe(secondCache);
    await first.shutdown();
    expect(clear.mock.contexts).toEqual([firstCache]);
    await second.onExit();
    expect(clear.mock.contexts).toEqual([firstCache, secondCache]);
    expect(bindingMocks.projectShutdown).toHaveBeenCalledTimes(1);
    expect(bindingMocks.projectOnExit).toHaveBeenCalledTimes(1);
  });

  it.each(["shutdown", "onExit"] as const)(
    "clears sources when native %s rejects and preserves the error",
    async (method) => {
      const clear = vi.spyOn(DiagnosticSources.prototype, "clear");
      const error = new Error("original native shutdown failure");
      const binding =
        method === "shutdown"
          ? bindingMocks.projectShutdown
          : bindingMocks.projectOnExit;
      binding.mockRejectedValue(error);
      const project = await createProject();
      await project.writeAllEntrypointsToDisk();

      await expect(project[method]()).rejects.toBe(error);
      expect(clear).toHaveBeenCalledTimes(1);
    },
  );

  it.each(["shutdown", "onExit"] as const)(
    "returns the original native %s promise and cleans only after it settles",
    async (method) => {
      const clear = vi.spyOn(DiagnosticSources.prototype, "clear");
      let resolve!: () => void;
      const pending = new Promise<void>((done) => {
        resolve = done;
      });
      const binding =
        method === "shutdown"
          ? bindingMocks.projectShutdown
          : bindingMocks.projectOnExit;
      binding.mockReturnValue(pending);
      const project = await createProject();
      await project.writeAllEntrypointsToDisk();

      expect(project[method]()).toBe(pending);
      expect(clear).not.toHaveBeenCalled();
      resolve();
      await pending;
      expect(clear).toHaveBeenCalledTimes(1);
    },
  );

  it.each(["shutdown", "onExit"] as const)(
    "preserves native %s synchronous exceptions while cleaning sources",
    async (method) => {
      const clear = vi.spyOn(DiagnosticSources.prototype, "clear");
      const error = new Error("original synchronous native failure");
      const binding =
        method === "shutdown"
          ? bindingMocks.projectShutdown
          : bindingMocks.projectOnExit;
      binding.mockImplementation(() => {
        throw error;
      });
      const project = await createProject();
      await project.writeAllEntrypointsToDisk();

      let thrown: unknown;
      try {
        project[method]();
      } catch (caught) {
        thrown = caught;
      }
      expect(thrown).toBe(error);
      expect(clear).toHaveBeenCalledTimes(1);
    },
  );
});

const subscriptions: {
  name: string;
  binding: ReturnType<typeof vi.fn>;
  open: (
    project: Project,
    endpoint: Endpoint,
  ) => AsyncIterableIterator<unknown> | Promise<AsyncIterableIterator<unknown>>;
  consumesInitial?: boolean;
}[] = [
  {
    name: "entrypoints",
    binding: bindingMocks.projectEntrypointsSubscribe,
    open: (project: Project) => project.entrypointsSubscribe(),
  },
  {
    name: "HMR events",
    binding: bindingMocks.projectHmrEvents,
    open: (project: Project) => project.hmrEvents("entry.js", "version"),
  },
  {
    name: "HMR identifiers",
    binding: bindingMocks.projectHmrIdentifiersSubscribe,
    open: (project: Project) => project.hmrIdentifiersSubscribe(),
  },
  {
    name: "update info",
    binding: bindingMocks.projectUpdateInfoSubscribe,
    open: (project: Project) => project.updateInfoSubscribe(100),
  },
  {
    name: "client changes",
    binding: bindingMocks.endpointClientChangedSubscribe,
    open: (_project: Project, endpoint: Endpoint) => endpoint.clientChanged(),
    consumesInitial: true,
  },
  {
    name: "server changes",
    binding: bindingMocks.endpointServerChangedSubscribe,
    open: (_project: Project, endpoint: Endpoint) =>
      endpoint.serverChanged(true),
    consumesInitial: true,
  },
];

describe.each(subscriptions)(
  "$name diagnostics",
  ({ name, binding, open, consumesInitial }) => {
    it("canonicalizes before buffering while preserving complete and edited diagnostics", async () => {
      let emit!: Callback;
      binding.mockImplementation((...args: unknown[]) => {
        emit = args.find((arg) => typeof arg === "function") as Callback;
        emit(undefined, diagnostic());
        return Promise.resolve(rootTask);
      });
      const intern = vi.spyOn(DiagnosticSources.prototype, "intern");
      const project = await createProject();
      const entrypoints = await project.writeAllEntrypointsToDisk();
      const projectCache = intern.mock.contexts[0];
      const subscription = await open(project, entrypoints.apps![0]);
      if (!consumesInitial) await subscription.next();
      intern.mockClear();
      const update = {
        type: "issues",
        resource: { path: "entry.js", headers: null },
        ...diagnostic(),
      };
      const expected = structuredClone(update);

      emit(undefined, update);

      // No next() is pending: canonicalization must happen before retaining the
      // result in the subscription buffer, rather than when it is consumed.
      expect(intern).toHaveBeenCalledWith(update);
      expect(intern.mock.contexts[0]).toBe(projectCache);
      const received = (await subscription.next()).value as TurbopackResult;
      expect(received.issues).toEqual(expected.issues);
      if (name !== "entrypoints") expect(received).toBe(update);
      expect(update).toEqual(expected);

      const changed = diagnostic("export const value = 'after';");
      emit(undefined, changed);
      const edited = (await subscription.next()).value as TurbopackResult;
      expect(edited.issues).toEqual(changed.issues);
      expect(received.issues).toEqual(expected.issues);
      await subscription.return!();
      expect(bindingMocks.rootTaskDispose).toHaveBeenCalledWith(rootTask);
    });
  },
);
