// Adapted from https://github.com/vercel/next.js/blob/canary/packages/next/src/client/dev/error-overlay/websocket.ts

type WebSocketMessage =
  | {
      type: "turbopack-connected";
    }
  | {
      type: "turbopack-message";
      data: Record<string, any>;
    };

let source: WebSocket;
const eventCallbacks: ((msg: WebSocketMessage) => void)[] = [];

function getSocketProtocol(): string {
  let protocol = location.protocol;
  return protocol === "http:" ? "ws" : "wss";
}

export function addMessageListener(cb: (msg: WebSocketMessage) => void) {
  eventCallbacks.push(cb);
}

export function sendMessage(data: any) {
  if (!source || source.readyState !== source.OPEN) return;
  return source.send(data);
}

export type HMROptions = {
  path: string;
};

let reconnections = 0;
let reloading = false;
let serverSessionId: number | null = null;

// This is not used by Next.js, but it is used by the standalone turbopack-cli
export function connectHMR(options: HMROptions) {
  function init() {
    if (source) source.close();

    console.log("[HMR] connecting...");

    function handleOnline() {
      reconnections = 0;
      window.console.log("[HMR] connected");
    }

    function handleMessage(event: MessageEvent<string>) {
      if (reloading) {
        return;
      }

      const msg = JSON.parse(event.data);

      if (msg.action === "turbopack-connected") {
        if (
          serverSessionId !== null &&
          serverSessionId !== msg.data.sessionId
        ) {
          window.location.reload();
          reloading = true;
          return;
        }

        serverSessionId = msg.data.sessionId;
      }

      if (msg.action === "reload") {
        window.location.reload();
        reloading = true;
        return;
      }

      if (["turbopack-connected", "turbopack-message"].includes(msg.action)) {
        for (const eventCallback of eventCallbacks) {
          eventCallback({ type: msg.action, data: msg.data });
        }
      }

      // TODO: handle rest msg.actions
    }

    let timer: ReturnType<typeof setTimeout>;
    function handleDisconnect() {
      source.onerror = null;
      source.onclose = null;
      source.close();
      reconnections++;
      // After 25 reconnects we'll want to reload the page as it indicates the dev server is no longer running.
      if (reconnections > 25) {
        reloading = true;
        window.location.reload();
        return;
      }

      clearTimeout(timer);
      // Try again after 5 seconds
      timer = setTimeout(init, reconnections > 5 ? 5000 : 1000);
    }

    const { hostname, port } = location;
    const protocol = getSocketProtocol();

    let url = `${protocol}://${hostname}:${port}`;

    source = new window.WebSocket(`${url}${options.path}`);
    source.onopen = handleOnline;
    source.onerror = handleDisconnect;
    source.onmessage = handleMessage;
  }

  init();
}
