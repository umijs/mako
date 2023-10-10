requireModule._interopreRequireWasm = (exports, wasmModulePath, importsObj) => {
  const request = fetch(wasmModulePath);
  if (typeof WebAssembly.instantiateStreaming === 'function') {
    return WebAssembly.instantiateStreaming(request, importsObj).then((res) =>
      Object.assign(exports, res.instance.exports),
    );
  }
  return request
    .then((body) => body.arrayBuffer())
    .then((bytes) => WebAssembly.instantiate(bytes, importsObj))
    .then((res) => Object.assign(exports, res.instance.exports));
};
