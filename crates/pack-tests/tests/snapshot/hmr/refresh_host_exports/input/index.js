import helpers from "@utoo/pack-runtime/react-refresh/internal/helpers";
import runtime from "@utoo/pack-runtime/react-refresh/internal/react-refresh-runtime.development";

// User code must still receive the configured legacy Symbol compatibility.
function userTypeof(value) {
  return typeof value;
}

globalThis.refreshHostExports = { helpers, runtime, userTypeof };
