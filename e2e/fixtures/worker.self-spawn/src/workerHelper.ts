function waitForMsgType(target, type) {
  return new Promise((resolve) => {
    target.addEventListener("message", function onMsg({ data }) {
      if (data == null || data.type !== type) return;
      target.removeEventListener("message", onMsg);
      resolve(data);
    });
  });
}

waitForMsgType(self, "spawn").then(async (data) => {
  const log = await import(".");
  log.default("spawn a web worker");
});

export async function startWorkers() {
  await Promise.all(
    Array.from({ length: 2 }, async () => {
      const worker = new Worker(new URL("./workerHelper", import.meta.url), {
        type: "classic",
      });
      worker.postMessage({ type: "spawn" });
    })
  );
}
