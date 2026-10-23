import type { ConfigComplete } from "@utoo/pack-shared";
import fs from "fs";
import type { Context, Next } from "hono";
import os from "os";
import path from "path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const serverMocks = vi.hoisted(() => ({
  serve: vi.fn((_options: unknown) => ({ listening: true })),
  proxy: vi.fn(async () => new Response("upstream response")),
  hotReloader: {
    start: vi.fn(async () => {}),
    close: vi.fn(async () => {}),
    getClientPaths: vi.fn(() => []),
    registerClient: vi.fn(),
    handleClientMessage: vi.fn(),
    unregisterClient: vi.fn(),
  },
}));

vi.mock("@hono/node-server", () => ({ serve: serverMocks.serve }));
vi.mock("@hono/node-ws", () => ({
  createNodeWebSocket: () => ({
    injectWebSocket: vi.fn(),
    upgradeWebSocket: () => (c: Context, next: Next) => {
      if (c.req.header("upgrade")?.toLowerCase() === "websocket") {
        return new Response("HMR upgrade", { status: 426 });
      }
      return next();
    },
  }),
}));
vi.mock("hono/proxy", () => ({ proxy: serverMocks.proxy }));
vi.mock("get-port", () => ({ default: async () => 43127 }));
vi.mock("../core/hmr", () => ({
  createHotReloader: async () => serverMocks.hotReloader,
}));
vi.mock("../config/webpackCompat", () => ({
  resolveBundleOptions: (options: unknown) => options,
}));
vi.mock("../utils/common", () => ({
  blockStdout: vi.fn(),
  getPackPath: () => "/unused-pack-path",
}));

import { type StartServerOptions, serve } from "../commands/dev";

const SPA_HTML = "<!doctype html><title>SPA shell</title>";
const OTHER_HTML = "<!doctype html><title>Other page</title>";
const JS_ASSET = 'console.log("real asset");';
const lifecycleEvents = [
  "SIGINT",
  "SIGTERM",
  "rejectionHandled",
  "uncaughtException",
  "unhandledRejection",
] as const;

let projectPath: string;
let distPath: string;
let originalTitle: string;
let previousListeners: Map<
  string,
  Set<ReturnType<typeof process.listeners>[number]>
>;

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubEnv("XCODE_PROFILE", "");
  vi.stubEnv("NODE_ENV", "test");
  vi.spyOn(console, "warn").mockImplementation(() => {});
  originalTitle = process.title;
  previousListeners = new Map(
    lifecycleEvents.map((event) => [event, new Set(process.listeners(event))]),
  );
  projectPath = fs.mkdtempSync(
    path.join(os.tmpdir(), "utoo-history-fallback-"),
  );
  distPath = path.join(projectPath, "dist");
  fs.mkdirSync(path.join(distPath, "pages"), { recursive: true });
  fs.writeFileSync(path.join(distPath, "index.html"), SPA_HTML);
  fs.writeFileSync(path.join(distPath, "app.js"), JS_ASSET);
  fs.writeFileSync(path.join(distPath, "other.html"), OTHER_HTML);
  fs.writeFileSync(path.join(distPath, "pages", "index.html"), OTHER_HTML);
});

afterEach(() => {
  for (const event of lifecycleEvents) {
    for (const listener of process.listeners(event)) {
      if (!previousListeners.get(event)?.has(listener)) {
        process.removeListener(event, listener);
      }
    }
  }
  process.title = originalTitle;
  fs.rmSync(projectPath, { force: true, recursive: true });
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
});

async function startServer(
  config: Partial<ConfigComplete> = {},
  serverOptions: Partial<StartServerOptions> = { historyApiFallback: true },
) {
  await serve({ config: { entry: [], ...config } }, projectPath, projectPath, {
    port: 3000,
    logServerInfo: false,
    ...serverOptions,
  });

  // Keep the real Hono routing and static-file middleware, without opening a port.
  const options = serverMocks.serve.mock.calls[0]?.[0] as unknown as {
    port: number;
    fetch: (request: Request) => Response | Promise<Response>;
  };
  expect(options.port).toBe(43127);

  return (pathname: string, init?: RequestInit) =>
    options.fetch(new Request(`http://localhost:43127${pathname}`, init));
}

describe("shared serve() history fallback option", () => {
  it.each([undefined, false])(
    "preserves unmatched navigation and API responses when fallback is %s",
    async (historyApiFallback) => {
      const request = await startServer(
        {},
        historyApiFallback === undefined ? {} : { historyApiFallback },
      );

      for (const { pathname, accept } of [
        { pathname: "/home", accept: "text/html" },
        { pathname: "/api/data", accept: "*/*" },
      ]) {
        const response = await request(pathname, { headers: { accept } });

        expect(response.status).toBe(404);
        expect(response.headers.get("vary")).toBeNull();
        expect(await response.text()).not.toBe(SPA_HTML);
      }
    },
  );

  it("serves the SPA shell when fallback is explicitly enabled", async () => {
    const request = await startServer({}, { historyApiFallback: true });
    const response = await request("/home", {
      headers: { accept: "text/html" },
    });

    expect(response.status).toBe(200);
    expect(await response.text()).toBe(SPA_HTML);
  });
});

describe("dev server SPA history fallback", () => {
  it.each(["GET", "HEAD"])(
    "varies both HTML and non-HTML responses by Accept for %s",
    async (method) => {
      const request = await startServer();

      for (const [accept, status] of [
        ["text/html", 200],
        ["application/json", 404],
      ] as const) {
        const response = await request("/home", {
          method,
          headers: { accept },
        });

        expect(response.status).toBe(status);
        expect(response.headers.get("vary")).toBe("Accept");
        await response.body?.cancel();
      }
    },
  );

  it.each([
    "/home",
    "/home/settings/profile",
    "/home?tab=details",
    "/v1.0/home",
  ])("serves index.html when refreshing %s", async (pathname) => {
    const request = await startServer();
    const response = await request(pathname, {
      headers: { accept: "text/html,application/xhtml+xml" },
    });

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toContain("text/html");
    expect(await response.text()).toBe(SPA_HTML);
  });

  it.each([undefined, "*/*", "text/html;q=0.9,*/*;q=0.8"])(
    "supports the default navigation Accept header %s",
    async (accept) => {
      const request = await startServer();
      const response = await request("/home", {
        headers: accept ? { accept } : undefined,
      });

      expect(response.status).toBe(200);
      expect(await response.text()).toBe(SPA_HTML);
    },
  );

  it.each([
    ["/app.js", JS_ASSET],
    ["/other.html", OTHER_HTML],
    ["/pages/", OTHER_HTML],
  ])(
    "serves existing files before fallback: %s",
    async (pathname, contents) => {
      const request = await startServer();
      const response = await request(pathname, {
        headers: { accept: "text/html" },
      });

      expect(response.status).toBe(200);
      expect(response.headers.get("vary")).toBeNull();
      expect(await response.text()).toBe(contents);
    },
  );

  it.each([
    "/missing.js",
    "/assets/missing.css",
    "/missing.html",
    "/missing.js/",
    "/missing%2Ejs",
  ])("does not replace missing files with HTML: %s", async (pathname) => {
    const request = await startServer();
    const response = await request(pathname, {
      headers: { accept: "text/html" },
    });

    expect(response.status).not.toBe(200);
    expect(response.headers.get("vary")).toBeNull();
    expect(await response.text()).not.toBe(SPA_HTML);
  });

  it.each([
    "application/json",
    "text/plain",
    "text/html;q=0",
    "text/html;q=0,*/*;q=1",
    "application/json,text/html;q=0",
    "application/json,*/*;q=0",
  ])("does not return HTML when Accept excludes it: %s", async (accept) => {
    const request = await startServer();
    const response = await request("/api/data", {
      headers: { accept },
    });

    expect(response.status).not.toBe(200);
    expect(await response.text()).not.toBe(SPA_HTML);
  });

  it("does not handle POST navigation requests", async () => {
    const request = await startServer();
    const response = await request("/home", {
      method: "POST",
      headers: { accept: "text/html" },
    });

    expect(response.status).toBe(405);
    expect(response.headers.get("allow")).toBe("GET, HEAD");
  });

  it("leaves missing routes unserved when index.html does not exist", async () => {
    fs.unlinkSync(path.join(distPath, "index.html"));
    const request = await startServer();
    const response = await request("/home", {
      headers: { accept: "text/html" },
    });

    expect(response.status).not.toBe(200);
    expect(await response.text()).not.toBe(SPA_HTML);
  });

  it("supports HEAD navigation under output.publicPath", async () => {
    const request = await startServer({ output: { publicPath: "/app/" } });
    const page = await request("/app/home/settings?tab=details", {
      headers: { accept: "text/html" },
    });
    expect(page.status).toBe(200);
    expect(await page.text()).toBe(SPA_HTML);

    const response = await request("/app/home/settings?tab=details", {
      method: "HEAD",
      headers: { accept: "text/html" },
    });

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toContain("text/html");
    expect(await response.text()).toBe("");

    const asset = await request("/app/app.js");
    expect(await asset.text()).toBe(JS_ASSET);
  });

  it("keeps configured HTTP proxies ahead of fallback", async () => {
    const request = await startServer({
      devServer: {
        proxy: [{ context: "/api", target: "http://upstream.invalid" }],
      },
    });
    const response = await request("/api/data?filter=active", {
      headers: { accept: "text/html" },
    });

    expect(response.status).toBe(200);
    expect(response.headers.get("vary")).toBeNull();
    expect(await response.text()).toBe("upstream response");
    expect(serverMocks.proxy).toHaveBeenCalledWith(
      "http://upstream.invalid/api/data?filter=active",
      expect.objectContaining({ method: "GET" }),
    );
  });

  it("keeps the HMR endpoint ahead of both proxy and fallback", async () => {
    const request = await startServer({
      devServer: {
        proxy: [
          { context: "/turbopack-hmr", target: "http://upstream.invalid" },
        ],
      },
    });
    const response = await request("/turbopack-hmr", {
      headers: { accept: "text/html", upgrade: "websocket" },
    });

    expect(response.status).toBe(426);
    expect(await response.text()).toBe("HMR upgrade");
    expect(serverMocks.proxy).not.toHaveBeenCalled();
  });

  it("does not return the SPA shell for an ordinary HMR endpoint request", async () => {
    const request = await startServer();
    const response = await request("/turbopack-hmr", {
      headers: { accept: "text/html" },
    });

    expect(response.status).not.toBe(200);
    expect(await response.text()).not.toBe(SPA_HTML);
    expect(serverMocks.proxy).not.toHaveBeenCalled();
  });
});
