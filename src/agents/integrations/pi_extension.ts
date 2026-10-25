// tokensave-managed extension: rewritten by `tokensave install --agent pi`,
// removed by `tokensave uninstall --agent pi`. Local edits are lost on the next
// install, so keep your own logic in a separate extension file.
//
// Pi has no shell hooks, so the three things Claude Code gets from its hooks
// are done here against the same hidden tokensave subcommands: the PreToolUse
// guardrail on code-shaped searches, the per-turn savings counter reset, and
// the session receipt.

import { spawn } from "node:child_process";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

const TOKENSAVE_BIN = "__TOKENSAVE_BIN__";

/** Tools whose input the guardrail classifies; everything else passes through. */
const GUARDED_TOOLS = new Set(["bash", "find", "grep"]);

interface TokensaveRun {
  /** Process exit code, or null when the process could not run at all. */
  code: number | null;
  stderr: string;
}

/** Runs a hidden tokensave subcommand, failing open on any spawn problem. */
function runTokensave(args: string[], stdin: string): Promise<TokensaveRun> {
  return new Promise((resolve) => {
    let stderr = "";
    let settled = false;
    const finish = (code: number | null) => {
      if (!settled) {
        settled = true;
        resolve({ code, stderr });
      }
    };
    const child = spawn(TOKENSAVE_BIN, args, { stdio: ["pipe", "ignore", "pipe"] });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });
    child.on("error", () => finish(null));
    child.on("close", (code) => finish(code));
    child.stdin.on("error", () => finish(null));
    child.stdin.end(stdin);
  });
}

/** Strips the terminal colour codes the receipt is printed with. */
function plainText(text: string): string {
  return text.replace(/\u001b\[[0-9;]*m/g, "").trim();
}

export default function (pi: ExtensionAPI) {
  pi.on("tool_call", async (event, ctx) => {
    if (!GUARDED_TOOLS.has(event.toolName)) {
      return;
    }
    const request = JSON.stringify({ toolName: event.toolName, input: event.input, cwd: ctx.cwd });
    const { code, stderr } = await runTokensave(["hook-pi-pre-tool-use"], request);
    if (code !== 2) {
      return;
    }
    return {
      block: true,
      reason: stderr.trim() || "This search looks like a code-symbol lookup; use the tokensave MCP tools.",
    };
  });

  pi.on("before_agent_start", async () => {
    await runTokensave(["hook-prompt-submit"], "");
  });

  pi.on("agent_settled", async (_event, ctx) => {
    const { stderr } = await runTokensave(["hook-pi-stop"], "");
    const receipt = plainText(stderr);
    if (receipt && ctx.hasUI) {
      ctx.ui.notify(receipt, "info");
    }
  });
}
