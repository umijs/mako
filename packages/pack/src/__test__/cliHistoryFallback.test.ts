import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const cliMocks = vi.hoisted(() => ({
  serve: vi.fn(async () => {}),
  projectOptions: { config: { entry: [] } },
}));

vi.mock("@utoo/pack", () => ({ serve: cliMocks.serve }));
vi.mock("../../../pack-cli/src/utils/common", () => ({
  resolveBuildOptions: async () => ({
    projectOptions: cliMocks.projectOptions,
    projectPath: "/fixture",
    rootPath: "/fixture",
  }),
}));

import { run } from "../../../pack-cli/src/index";

let originalArgv: string[];

beforeEach(() => {
  vi.clearAllMocks();
  originalArgv = process.argv;
});

afterEach(() => {
  process.argv = originalArgv;
});

describe("CLI SPA history fallback", () => {
  it.each([
    [[], true],
    [["--no-history-api-fallback"], false],
  ] as const)("passes %j as historyApiFallback=%s", async (flags, enabled) => {
    process.argv = ["node", "up", "dev", ...flags];

    await run();

    expect(cliMocks.serve).toHaveBeenCalledExactlyOnceWith(
      cliMocks.projectOptions,
      "/fixture",
      "/fixture",
      {
        logServerInfo: true,
        port: 3000,
        historyApiFallback: enabled,
      },
    );
  });
});
