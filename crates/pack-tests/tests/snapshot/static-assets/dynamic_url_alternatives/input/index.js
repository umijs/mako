globalThis.createWorker = ({ classWorkerURL, ...options } = {}) => {
  if (classWorkerURL) {
    return new Worker(new URL(classWorkerURL, import.meta.url), options);
  }
};
