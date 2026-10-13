// Run against either a published package or a local build:
// node scripts/check-windows-provider-resolution.cjs --pack-path <package directory>
// NAPI_RS_NATIVE_LIBRARY_PATH may select a freshly built native binding.
const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");

const ENTRY_MARKER = "ABSOLUTE_PROVIDER_ENTRY";
const PROVIDER_MARKER = "ABSOLUTE_PROCESS_PROVIDER_RESOLVED";
const RESULT_PREFIX = "__WINDOWS_PROVIDER_RESOLUTION__";
const TIMEOUT_MS = 90_000;

function option(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

function packagePath() {
  const supplied = option("--pack-path");
  if (supplied) {
    const resolved = path.resolve(supplied);
    return resolved.endsWith("package.json")
      ? path.dirname(resolved)
      : resolved;
  }
  return path.resolve(__dirname, "..");
}

function javascriptAssets(directory, prefix = "") {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const relativePath = path.posix.join(prefix, entry.name);
    if (entry.isDirectory()) {
      return javascriptAssets(path.join(directory, entry.name), relativePath);
    }
    return entry.name.endsWith(".js") ? [relativePath] : [];
  });
}

function checkWebSocket(port) {
  return new Promise((resolve, reject) => {
    const socket = net.connect(port, "127.0.0.1");
    let response = "";
    let finished = false;
    const finish = (error) => {
      if (finished) return;
      finished = true;
      clearTimeout(timeout);
      socket.destroy();
      if (error) reject(error);
      else resolve();
    };
    const timeout = setTimeout(
      () => finish(new Error("HMR WebSocket handshake timed out")),
      10_000,
    );
    socket.on("connect", () => {
      socket.write(
        `GET /turbopack-hmr HTTP/1.1\r\n` +
          `Host: 127.0.0.1:${port}\r\n` +
          "Connection: Upgrade\r\n" +
          "Upgrade: websocket\r\n" +
          "Sec-WebSocket-Version: 13\r\n" +
          "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
      );
    });
    socket.on("data", (chunk) => {
      response += chunk.toString();
      if (!response.includes("\r\n\r\n")) return;
      finish(
        response.startsWith("HTTP/1.1 101 ")
          ? undefined
          : new Error(`Unexpected HMR WebSocket response: ${response}`),
      );
    });
    socket.on("error", finish);
    socket.on("end", () => {
      if (!response.includes("\r\n\r\n")) {
        finish(
          new Error("HMR WebSocket closed before the handshake completed"),
        );
      }
    });
  });
}

async function childMain() {
  const projectPath = option("--project-path");
  assert(projectPath, "The fixture project path is required");
  const providerPath = path.join(projectPath, "process-polyfill.js");
  fs.writeFileSync(
    providerPath,
    `export default { env: { PACK_PROVIDER_MARKER: ${JSON.stringify(
      PROVIDER_MARKER,
    )} } };\n`,
  );
  fs.writeFileSync(
    path.join(projectPath, "index.js"),
    `console.log(${JSON.stringify(
      ENTRY_MARKER,
    )}, process.env.PACK_PROVIDER_MARKER);\n`,
  );
  fs.writeFileSync(
    path.join(projectPath, "package.json"),
    JSON.stringify({
      name: "windows-provider-resolution-fixture",
      private: true,
    }),
  );

  const packPath = packagePath();
  const { serve } = require(packPath);
  const normalizedProjectPath = projectPath.replace(/\\/g, "/");
  let readyContext;
  await serve(
    {
      config: {
        entry: [{ import: "./index.js", name: "main" }],
        output: { path: "./dist", clean: true },
        provider: { process: providerPath },
        // Match Umi's root alias for POSIX absolute provider imports. Windows
        // provider requests retain their drive letter and native separators.
        resolve: {
          alias: {
            [`${normalizedProjectPath}/*`]: `${normalizedProjectPath}/*`,
          },
        },
        optimization: { minify: false, moduleIds: "named" },
        devServer: { hot: true },
        persistentCaching: false,
        stats: false,
      },
    },
    projectPath,
    projectPath,
    {
      hostname: "127.0.0.1",
      port: 0,
      logServerInfo: false,
      onReady(context) {
        readyContext = context;
      },
    },
  );
  assert(readyContext, "serve() did not report readiness");
  assert(
    readyContext.port > 0,
    "serve() did not report the actual listening port",
  );
  // The readiness hook returns initial entry paths; their referenced runtime
  // chunks also have to be served successfully.
  const clientPaths = [
    ...new Set([
      ...readyContext.clientPaths.filter((asset) => asset.endsWith(".js")),
      ...javascriptAssets(path.join(projectPath, "dist")),
    ]),
  ];
  assert(
    clientPaths.length > 0,
    "The development build did not emit JavaScript",
  );
  const sources = await Promise.all(
    clientPaths.map(async (asset) => {
      const response = await fetch(
        new URL(asset, `http://127.0.0.1:${readyContext.port}/`),
        { signal: AbortSignal.timeout(10_000) },
      );
      assert.equal(response.status, 200, `Could not fetch ${asset}`);
      return response.text();
    }),
  );
  const source = sources.join("\n");
  assert(source.includes(ENTRY_MARKER), "The application entry was not served");
  assert(
    source.includes(PROVIDER_MARKER),
    "The absolute process provider was not included in the served build",
  );
  assert(
    source.includes("/turbopack-hmr"),
    "The embedded HMR client is missing",
  );
  assert(
    source.includes("$RefreshHelpers$") &&
      source.includes("injectIntoGlobalHook"),
    "The embedded React Refresh runtime is missing",
  );
  await checkWebSocket(readyContext.port);

  console.log(
    `${RESULT_PREFIX}${JSON.stringify({
      platform: process.platform,
      packVersion: require(path.join(packPath, "package.json")).version,
      absoluteProvider: true,
      embeddedHmr: true,
      embeddedReactRefresh: true,
      hmrWebSocket: true,
    })}`,
  );
  // Windows has no graceful POSIX signal delivery. There is no persistent
  // cache in this fixture, and exiting releases its listener and sockets.
  if (process.platform === "win32") process.exit(0);
  process.kill(process.pid, "SIGTERM");
}

async function parentMain() {
  const projectPath = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), "utoo-windows-provider-")),
  );
  try {
    await new Promise((resolve, reject) => {
      const child = spawn(
        process.execPath,
        [
          __filename,
          "--child",
          "--pack-path",
          packagePath(),
          "--project-path",
          projectPath,
        ],
        { cwd: projectPath, stdio: ["ignore", "pipe", "pipe"] },
      );
      let stdout = "";
      let stderr = "";
      let timedOut = false;
      let forceKill;
      child.stdout.on("data", (chunk) => {
        stdout += chunk;
      });
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
      });
      const timeout = setTimeout(() => {
        timedOut = true;
        child.kill("SIGTERM");
        forceKill = setTimeout(() => child.kill("SIGKILL"), 5_000);
      }, TIMEOUT_MS);
      child.on("error", (error) => {
        clearTimeout(timeout);
        clearTimeout(forceKill);
        reject(error);
      });
      child.on("exit", (code, signal) => {
        clearTimeout(timeout);
        clearTimeout(forceKill);
        const result = stdout
          .split(/\r?\n/)
          .find((line) => line.startsWith(RESULT_PREFIX));
        if (timedOut || !result || (code !== 0 && signal !== "SIGTERM")) {
          reject(
            new Error(
              `Provider fixture ${
                timedOut ? "timed out" : "failed"
              } (code ${code}, signal ${signal})\n${stderr}\n${stdout}`,
            ),
          );
          return;
        }
        console.log(result);
        resolve();
      });
    });
  } finally {
    fs.rmSync(projectPath, { recursive: true, force: true, maxRetries: 5 });
  }
}

(process.argv.includes("--child") ? childMain() : parentMain()).catch(
  (error) => {
    console.error(error);
    process.exit(1);
  },
);
