import type { NapiWrittenEndpoint } from "../binding";

export type WrittenEndpointPaths = Pick<
  NapiWrittenEndpoint,
  "clientPaths" | "serverPaths"
>;

export function pickWrittenEndpointPaths(
  written: WrittenEndpointPaths,
): WrittenEndpointPaths {
  // Native write results also contain diagnostics, including complete source
  // files. Keep those out of caches that only need the output paths.
  return {
    clientPaths: written.clientPaths,
    serverPaths: written.serverPaths,
  };
}
