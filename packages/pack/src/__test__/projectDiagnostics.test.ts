import { afterEach, beforeEach, expect, it, vi } from "vitest";
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

const factory = projectFactory();
const createProject = () => factory(options, {});

beforeEach(() => {
  vi.resetAllMocks();
  bindingMocks.projectNew.mockResolvedValue({ __napiType: "Project" });
  bindingMocks.projectWriteAllEntrypointsToDisk.mockImplementation(async () =>
    nativeEntrypoints(),
  );
});

afterEach(() => vi.restoreAllMocks());

it("preserves complete production writes, shares endpoint sources and isolates projects", async () => {
  const intern = vi.spyOn(DiagnosticSources.prototype, "intern");
  const native = nativeEntrypoints();
  const expected = structuredClone(native);
  bindingMocks.projectWriteAllEntrypointsToDisk.mockResolvedValue(native);
  const project = await createProject();
  const result = await project.writeAllEntrypointsToDisk();
  const sources = intern.mock.contexts[0];

  expect(result).toEqual({
    ...expected,
    apps: result.apps,
    libraries: result.libraries,
  });
  expect(native).toEqual(expected);
  for (const endpoint of [...result.apps!, ...result.libraries!]) {
    const written = writtenEndpoint();
    const before = structuredClone(written);
    bindingMocks.endpointWriteToDisk.mockResolvedValueOnce(written);
    expect(await endpoint.writeToDisk()).toBe(written);
    expect(written).toEqual(before);
    expect(intern.mock.contexts.at(-1)).toBe(sources);
  }
  const other = await createProject();
  await other.writeAllEntrypointsToDisk();
  expect(intern.mock.contexts.at(-1)).not.toBe(sources);
});

it.each(["shutdown", "onExit"] as const)(
  "disposes sources before native %s and preserves late results and native behavior",
  async (method) => {
    const intern = vi.spyOn(DiagnosticSources.prototype, "intern");
    const dispose = vi.spyOn(DiagnosticSources.prototype, "dispose");
    const project = await createProject();
    const entrypoints = await project.writeAllEntrypointsToDisk();
    const sources = intern.mock.contexts[0];
    let emit!: Callback;
    bindingMocks.projectHmrEvents.mockImplementation(
      (_project, _identifier, callback: Callback) => {
        emit = callback;
        emit(undefined, diagnostic());
        return Promise.resolve(rootTask);
      },
    );
    const subscription = project.hmrEvents("entry.js");
    await subscription.next();
    let finishWrite!: (value: ReturnType<typeof writtenEndpoint>) => void;
    bindingMocks.endpointWriteToDisk.mockReturnValueOnce(
      new Promise((resolve) => {
        finishWrite = resolve;
      }),
    );
    const writing = entrypoints.apps![0].writeToDisk();
    let finishAll!: (value: ReturnType<typeof nativeEntrypoints>) => void;
    bindingMocks.projectWriteAllEntrypointsToDisk.mockReturnValueOnce(
      new Promise((resolve) => {
        finishAll = resolve;
      }),
    );
    const writingAll = project.writeAllEntrypointsToDisk();
    const key = method === "shutdown" ? "projectShutdown" : "projectOnExit";
    const original = bindingMocks[key];
    // vi.fn observes returned Promises itself; use a plain native stub here.
    const native = (close: () => Promise<void>) =>
      Object.assign(bindingMocks, { [key]: close });
    try {
      let resolve!: () => void;
      const pending = new Promise<void>((done) => {
        resolve = done;
      });
      const pendingThen = vi.spyOn(pending, "then");
      native(() => {
        expect(dispose.mock.contexts).toEqual([sources]);
        return pending;
      });

      expect(project[method]()).toBe(pending);
      expect(pendingThen).not.toHaveBeenCalled();
      expect(dispose.mock.contexts).toEqual([sources]);
      const beforeLate = intern.mock.calls.length;
      const late = diagnostic("after close began");
      const lateWrite = writtenEndpoint("after close began");
      const lateAll = nativeEntrypoints("after close began");
      emit(undefined, late);
      finishWrite(lateWrite);
      finishAll(lateAll);
      expect((await subscription.next()).value).toBe(late);
      expect(await writing).toBe(lateWrite);
      const all = await writingAll;
      expect(all).toEqual({
        ...lateAll,
        apps: all.apps,
        libraries: all.libraries,
      });
      expect(
        intern.mock.contexts
          .slice(beforeLate)
          .every((cache) => cache === sources),
      ).toBe(true);
      expect(dispose.mock.invocationCallOrder[0]).toBeLessThan(
        intern.mock.invocationCallOrder[beforeLate],
      );
      resolve();
      await pending;
      const afterClose = diagnostic("after close completed");
      emit(undefined, afterClose);
      expect((await subscription.next()).value).toBe(afterClose);

      const error = new Error("original native failure");
      const rejected = Promise.reject(error);
      const rejectedThen = vi.spyOn(rejected, "then");
      native(() => rejected);
      expect(project[method]()).toBe(rejected);
      expect(rejectedThen).not.toHaveBeenCalled();
      expect(dispose.mock.contexts).toEqual([sources, sources]);
      await expect(rejected).rejects.toBe(error);

      native(() => {
        expect(dispose.mock.contexts).toEqual([sources, sources, sources]);
        throw error;
      });
      let thrown: unknown;
      try {
        project[method]();
      } catch (caught) {
        thrown = caught;
      }
      expect(thrown).toBe(error);
      expect(dispose.mock.contexts).toEqual([sources, sources, sources]);
    } finally {
      bindingMocks[key] = original;
      await subscription.return!();
      expect(bindingMocks.rootTaskDispose).toHaveBeenCalledWith(rootTask);
    }
  },
);

type OpenSubscription = (
  project: Project,
  endpoint: Endpoint,
) => AsyncIterableIterator<unknown> | Promise<AsyncIterableIterator<unknown>>;
const subscriptions = {
  projectEntrypointsSubscribe: (p) => p.entrypointsSubscribe(),
  projectHmrEvents: (p) => p.hmrEvents("entry.js", "version"),
  projectHmrIdentifiersSubscribe: (p) => p.hmrIdentifiersSubscribe(),
  projectUpdateInfoSubscribe: (p) => p.updateInfoSubscribe(100),
  endpointClientChangedSubscribe: (_, e) => e.clientChanged(),
  endpointServerChangedSubscribe: (_, e) => e.serverChanged(true),
} satisfies Record<string, OpenSubscription>;

it.each(Object.entries(subscriptions))(
  "%s interns before buffering and preserves diagnostics",
  async (name, open) => {
    const binding = bindingMocks[name as keyof typeof subscriptions];
    let emit!: Callback;
    binding.mockImplementation((...args: unknown[]) => {
      emit = args.find((arg) => typeof arg === "function") as Callback;
      emit(undefined, diagnostic());
      return Promise.resolve(rootTask);
    });
    const intern = vi.spyOn(DiagnosticSources.prototype, "intern");
    const project = await createProject();
    const entrypoints = await project.writeAllEntrypointsToDisk();
    const sources = intern.mock.contexts[0];
    const subscription = await open(project, entrypoints.apps![0]);
    if (!name.startsWith("endpoint")) await subscription.next();
    intern.mockClear();
    const update = {
      type: "issues",
      resource: { path: "entry.js", headers: null },
      ...diagnostic(),
    };
    const expected = structuredClone(update);

    emit(undefined, update);
    // No next() is pending: intern before the subscription retains the result.
    expect(intern).toHaveBeenCalledWith(update);
    expect(intern.mock.contexts[0]).toBe(sources);
    const received = (await subscription.next()).value as TurbopackResult;
    expect(received.issues).toEqual(expected.issues);
    if (name !== "projectEntrypointsSubscribe") expect(received).toBe(update);
    expect(update).toEqual(expected);

    const changed = diagnostic("export const value = 'after';");
    emit(undefined, changed);
    expect(
      ((await subscription.next()).value as TurbopackResult).issues,
    ).toEqual(changed.issues);
    expect(received.issues).toEqual(expected.issues);
    await subscription.return!();
    expect(bindingMocks.rootTaskDispose).toHaveBeenCalledWith(rootTask);
  },
);
