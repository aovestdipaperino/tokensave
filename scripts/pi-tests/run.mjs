// Run only Tokensave's offline lifecycle fixtures inside a prepared Pi checkout.
import { mkdtemp, copyFile, rm, access } from "node:fs/promises";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
if (process.argv.length !== 3) throw new Error("Usage: node scripts/pi-tests/run.mjs <prepared-pi-checkout>");
const pi = resolve(process.argv[2]);
const pkg = join(pi, "packages", "coding-agent");
const cli = join(pi, "node_modules", "vitest", "vitest.mjs");
await access(cli);
await access(join(pkg, "test", "suite", "harness.ts"));
const dir = await mkdtemp(join(pkg, "test", "suite", "tokensave-"));
try {
  await copyFile(join(root, "tests", "pi", "lifecycle.test.ts"), join(dir, "lifecycle.test.ts"));
  await copyFile(join(root, "tests", "pi", "evaluation_observer.test.ts"), join(dir, "evaluation_observer.test.ts"));
  await copyFile(join(root, "scripts", "pi-eval", "observer.mjs"), join(dir, "evaluation-observer.mjs"));
  await copyFile(join(root, "src", "agents", "integrations", "pi_extension.ts"), join(dir, "tokensave-extension.ts"));
  const code = await new Promise((resolveRun, reject) => {
    const child = spawn(process.execPath, [cli, "run", "--pool=threads", "--maxWorkers=1", relative(pkg, join(dir, "lifecycle.test.ts")), relative(pkg, join(dir, "evaluation_observer.test.ts"))], {
      cwd: pkg, stdio: "inherit", env: { ...process.env, PI_OFFLINE: "1", PI_NO_LOCAL_LLM: "1" },
    });
    const timer = setTimeout(() => child.kill("SIGKILL"), 120_000);
    const interrupt = () => child.kill("SIGINT");
    process.once("SIGINT", interrupt);
    process.once("SIGTERM", interrupt);
    const cleanup = () => {
      clearTimeout(timer);
      process.removeListener("SIGINT", interrupt);
      process.removeListener("SIGTERM", interrupt);
    };
    child.once("error", error => { cleanup(); reject(error); });
    child.once("close", (status) => { cleanup(); resolveRun(status ?? 1); });
  });
  process.exitCode = code;
} finally {
  await rm(dir, { recursive: true, force: true });
}
