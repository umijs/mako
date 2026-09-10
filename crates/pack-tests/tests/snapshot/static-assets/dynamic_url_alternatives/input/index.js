globalThis.createWorker = ({ classWorkerURL, ...options } = {}) =>
  classWorkerURL
    ? new Worker(new URL(classWorkerURL, import.meta.url), options)
    : new Worker(new URL("./worker.js", import.meta.url), options);

globalThis.optionalURL = (url = undefined) => new URL(url, import.meta.url);
globalThis.fallbackURL = (url) =>
  new URL(url || "./fallback.txt", import.meta.url);
globalThis.dynamicURL = (url) => new URL(url, import.meta.url);
globalThis.staticURL = () => new URL("./fallback.txt", import.meta.url);
globalThis.boundedURL = (name) =>
  new URL(`./assets/${name}.txt`, import.meta.url);
