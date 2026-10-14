import fs from "fs";
import http from "http";
import os from "os";
import path from "path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { NapiIssue, NapiWrittenEndpoint } from "../binding";
import type { Endpoint, RawEntrypoints } from "../core/types";

const projectMocks = vi.hoisted(() => ({ projectFactory: vi.fn() }));

vi.mock("../core/project", () => ({
  projectFactory: projectMocks.projectFactory,
}));
vi.mock("../binding", () => ({}));

import { type DevServerHandle, serve } from "../commands/dev";
import { HtmlPlugin } from "../plugins/HtmlPlugin";

const handles: DevServerHandle[] = [];
const closeStreams: Array<() => Promise<unknown>> = [];
let projectPath: string;
let oldTitle: string;
let oldNodeEnv: string | undefined;

function createStream<T>() {
  const values: T[] = [];
  let pending: ((result: IteratorResult<T>) => void) | undefined;
  let closed = false;
  const stream: AsyncIterableIterator<T> = {
    [Symbol.asyncIterator]() {
      return this;
    },
    next() {
      if (values.length > 0) {
        return Promise.resolve({
          done: false as const,
          value: values.shift()!,
        });
      }
      if (closed) {
        return Promise.resolve({ done: true as const, value: undefined });
      }
      return new Promise((resolve) => {
        pending = resolve;
      });
    },
    async return() {
      closed = true;
      pending?.({ done: true, value: undefined });
      pending = undefined;
      return { done: true, value: undefined };
    },
  };
  closeStreams.push(() => stream.return!());
  return {
    stream,
    push(value: T) {
      if (pending) {
        const resolve = pending;
        pending = undefined;
        resolve({ done: false, value });
      } else {
        values.push(value);
      }
    },
  };
}

function createIssue(severity: "warning" | "error" | "fatal"): NapiIssue {
  return {
    severity,
    stage: "build",
    filePath: "[project]/entry.js",
    title: { type: "text", value: "Publishing fixture diagnostic" },
    description: { type: "text", value: "The current entry cannot compile" },
    documentationLink: "",
    importTraces: [],
  };
}

function createProject() {
  const state = { marker: "A", issues: [] as NapiIssue[] };
  const outputDir = path.join(projectPath, "dist");
  const nativeResults: TurbopackResult<RawEntrypoints>[] = [];
  const clientChanges = createStream<TurbopackResult>();
  const serverChanges = createStream<TurbopackResult>();
  const entrypoints = createStream<TurbopackResult<RawEntrypoints>>();
  fs.writeFileSync(path.join(projectPath, "entry.js"), "export default 1;\n");

  function writeAsset(): NapiWrittenEndpoint {
    const clientPath = `main.${state.marker.toLowerCase().repeat(8)}.js`;
    fs.mkdirSync(outputDir, { recursive: true });
    fs.writeFileSync(
      path.join(outputDir, clientPath),
      `window.marker = ${JSON.stringify(state.marker)};`,
    );
    return {
      type: "nodejs",
      entryPath: "server.js",
      clientPaths: [clientPath],
      serverPaths: [],
      config: {},
    };
  }

  function createEndpoint(): Endpoint {
    return {
      writeToDisk: vi.fn(async () => ({ ...writeAsset(), issues: [] })),
      clientChanged: vi.fn(async () => clientChanges.stream),
      serverChanged: vi.fn(async () => serverChanges.stream),
    };
  }

  const subscribedEndpoint = createEndpoint();
  entrypoints.push({ apps: [subscribedEndpoint], libraries: [], issues: [] });
  const project = {
    entrypointsSubscribe: () => entrypoints.stream,
    writeAllEntrypointsToDisk: vi.fn(async () => {
      // The binding creates new EndpointImpl objects for every all-write result.
      const result = {
        apps: [createEndpoint()],
        libraries: [],
        appPaths: [writeAsset()],
        libraryPaths: [],
        issues: [...state.issues],
      };
      nativeResults.push(result);
      return result;
    }),
    updateInfoSubscribe: () => (async function* () {})(),
    onExit: vi.fn(async () => {}),
    shutdown: vi.fn(async () => {}),
  };
  projectMocks.projectFactory.mockReturnValue(async () => project);
  return { clientChanges, nativeResults, project, state, subscribedEndpoint };
}

async function getAvailablePort(): Promise<number> {
  const server = http.createServer();
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  await new Promise<void>((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
  if (!address || typeof address === "string") {
    throw new Error("Expected a TCP listener address");
  }
  return address.port;
}

async function startServer() {
  const handle = await serve(
    {
      tracing: false,
      config: {
        entry: [
          {
            name: "main",
            import: "./entry.js",
            html: { filename: "index.html" },
          },
        ],
        output: { path: "./dist", clean: true },
        persistentCaching: false,
      },
    },
    projectPath,
    projectPath,
    {
      port: await getAvailablePort(),
      hostname: "127.0.0.1",
      handleSignals: false,
      returnHandle: true,
      logServerInfo: false,
    },
  );
  handles.push(handle);
  return handle;
}

async function readHttp(handle: DevServerHandle, pathname = "/index.html") {
  const response = await fetch(
    `http://${handle.hostname}:${handle.port}${pathname}`,
  );
  expect(response.status).toBe(200);
  return response.text();
}

beforeEach(() => {
  projectPath = fs.mkdtempSync(path.join(os.tmpdir(), "utoo-dev-publishing-"));
  oldTitle = process.title;
  oldNodeEnv = process.env.NODE_ENV;
  projectMocks.projectFactory.mockReset();
});

afterEach(async () => {
  await Promise.allSettled(handles.splice(0).map((handle) => handle.close()));
  await Promise.all(closeStreams.splice(0).map((close) => close()));
  fs.rmSync(projectPath, { recursive: true, force: true });
  process.title = oldTitle;
  if (oldNodeEnv === undefined) delete process.env.NODE_ENV;
  else process.env.NODE_ENV = oldNodeEnv;
  vi.restoreAllMocks();
});

describe("dev server publishing pipeline", () => {
  it("publishes HTML with fresh content hashes and preserves the native result", async () => {
    const fixture = createProject();
    const handle = await startServer();
    expect(await readHttp(handle)).toContain('src="main.aaaaaaaa.js"');
    fixture.state.marker = "B";
    fixture.state.issues = [createIssue("warning")];

    const written = await handle.writeAllEntrypointsToDisk();

    expect(written).toBe(fixture.nativeResults[0]);
    expect(written.issues).toEqual(fixture.state.issues);
    expect(written.apps?.[0]).not.toBe(fixture.subscribedEndpoint);
    expect(written.appPaths?.[0].clientPaths).toEqual(["main.bbbbbbbb.js"]);
    const html = await readHttp(handle);
    expect(html).toContain('src="main.bbbbbbbb.js"');
    expect(html).not.toContain("main.aaaaaaaa.js");
    expect(await readHttp(handle, "/main.bbbbbbbb.js")).toContain('"B"');
  });

  it("keeps subscribed endpoint HTML current after a full write returns new endpoints", async () => {
    const fixture = createProject();
    const handle = await startServer();
    fixture.state.marker = "B";
    await handle.writeAllEntrypointsToDisk();

    fixture.state.marker = "C";
    fixture.clientChanges.push({ issues: [] });

    await vi.waitFor(() => {
      const html = fs.readFileSync(
        path.join(projectPath, "dist/index.html"),
        "utf8",
      );
      expect(html).toContain('src="main.cccccccc.js"');
      expect(html).not.toContain("main.bbbbbbbb.js");
    });
    expect(await readHttp(handle)).toContain('src="main.cccccccc.js"');
    expect(await readHttp(handle, "/main.cccccccc.js")).toContain('"C"');
  });

  it("awaits HTML generation, propagates its failure, and can publish again", async () => {
    const fixture = createProject();
    const handle = await startServer();
    const error = new Error("HTML output is temporarily unavailable");
    let rejectHtml!: (reason: Error) => void;
    const htmlGate = new Promise<void>((_resolve, reject) => {
      rejectHtml = reject;
    });
    void htmlGate.catch(() => {});
    const started = vi.fn();
    vi.spyOn(HtmlPlugin.prototype, "generate").mockImplementationOnce(
      async () => {
        started();
        await htmlGate;
      },
    );
    fixture.state.marker = "B";
    let settled = false;
    const outcome = handle.writeAllEntrypointsToDisk().then(
      (value) => {
        settled = true;
        return { status: "fulfilled", value };
      },
      (reason) => {
        settled = true;
        return { status: "rejected", reason };
      },
    );

    try {
      await vi.waitFor(() => expect(started).toHaveBeenCalledOnce());
      expect(settled).toBe(false);
      expect(await readHttp(handle)).toContain('src="main.aaaaaaaa.js"');
    } finally {
      rejectHtml(error);
    }
    expect(await outcome).toEqual({ status: "rejected", reason: error });

    fixture.state.marker = "C";
    await handle.writeAllEntrypointsToDisk();
    expect(await readHttp(handle)).toContain('src="main.cccccccc.js"');
  });

  it.each(["error", "fatal"] as const)(
    "rejects %s diagnostics before publishing HTML and recovers on success",
    async (severity) => {
      const fixture = createProject();
      const handle = await startServer();
      fixture.state.marker = "B";
      fixture.state.issues = [createIssue(severity)];

      await expect(handle.writeAllEntrypointsToDisk()).rejects.toMatchObject({
        name: "ModuleBuildError",
        message: expect.stringContaining("Publishing fixture diagnostic"),
      });
      expect(await readHttp(handle)).toContain('src="main.aaaaaaaa.js"');

      fixture.state.marker = "C";
      fixture.state.issues = [];
      const written = await handle.writeAllEntrypointsToDisk();
      expect(written.issues).toEqual([]);
      expect(await readHttp(handle)).toContain('src="main.cccccccc.js"');
    },
  );
});
