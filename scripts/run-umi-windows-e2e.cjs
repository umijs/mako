const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const STEP_NAME = "e2e:example:with-use-model:dev:utoopack";

function extractRun(source) {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const names = lines.flatMap((line, index) =>
    line.trim() === `- name: ${STEP_NAME}` ? [index] : []
  );
  assert.equal(names.length, 1, `Expected one ${STEP_NAME} step`);
  const stepStart = names[0];
  const stepIndent =
    lines[stepStart].length - lines[stepStart].trimStart().length;
  let stepEnd = stepStart + 1;
  while (
    stepEnd < lines.length &&
    (!lines[stepEnd].trim() ||
      lines[stepEnd].length - lines[stepEnd].trimStart().length > stepIndent)
  ) {
    stepEnd += 1;
  }
  const step = lines.slice(stepStart, stepEnd);
  assert(
    step.some((line) => line.trim() === "shell: bash"),
    "Expected a Bash step"
  );
  const runIndex = step.findIndex((line) => line.trim() === "run: |");
  assert(
    runIndex !== -1,
    "Expected the E2E script in a literal YAML run block"
  );
  const runIndent = step[runIndex].length - step[runIndex].trimStart().length;
  const body = [];
  for (const line of step.slice(runIndex + 1)) {
    if (line.trim()) {
      const indent = line.length - line.trimStart().length;
      if (indent <= runIndent) break;
      assert(indent >= runIndent + 2, "Invalid E2E run block indentation");
    }
    body.push(line.slice(runIndent + 2));
  }
  const script = `${body.join("\n").trimEnd()}\n`;
  assert(script.trim(), "The Umi Windows E2E run block is empty");
  return script;
}

function main() {
  const args = process.argv.slice(2);
  const index = args.indexOf("--umi-path");
  assert(
    index !== -1 && args[index + 1],
    "Usage: --umi-path <checkout> [--check]"
  );
  const umiPath = path.resolve(args[index + 1]);
  const workflowPath = path.join(
    umiPath,
    ".github/workflows/e2e-utoopack-windows.yml"
  );
  const script = extractRun(fs.readFileSync(workflowPath, "utf8"));
  const bash =
    process.env.BASH ||
    (process.platform === "win32"
      ? path.join(
          process.env.ProgramFiles || "C:\\Program Files",
          "Git/bin/bash.exe"
        )
      : "bash");
  const check = spawnSync(bash, ["--noprofile", "--norc", "-n"], {
    cwd: umiPath,
    input: script,
    encoding: "utf8",
  });
  if (check.error) throw check.error;
  assert.equal(
    check.status,
    0,
    `Invalid Umi E2E Bash script:\n${check.stderr}`
  );
  console.log(`Verified ${STEP_NAME} from ${workflowPath}`);
  if (args.includes("--check")) return;

  const result = spawnSync(
    bash,
    ["--noprofile", "--norc", "-e", "-o", "pipefail"],
    {
      cwd: umiPath,
      input: script,
      stdio: ["pipe", "inherit", "inherit"],
      env: { ...process.env, FORCE_UTOOPACK: "1" },
    }
  );
  if (result.error) throw result.error;
  process.exitCode = result.status === null ? 1 : result.status;
}

if (require.main === module) main();

module.exports = { extractRun };
