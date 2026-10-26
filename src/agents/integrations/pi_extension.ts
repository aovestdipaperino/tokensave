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

// A separate section also works with Pi's custom prompt prefix. Keep this
// conditional: MCP readiness and adapter discovery can happen after our handler.
const RETRIEVAL_GUIDANCE = `For repository code exploration, prefer tokensave when its tools are reachable and an appropriate project index is available.
Start with tokensave_context for a question, tokensave_search for a symbol, tokensave_callers or tokensave_callees for relationships, and tokensave_read for indexed source ranges. Use tokensave_files for indexed file discovery.
With Pi's built-in MCP, use the declared mcp__tokensave__<tool> names. With pi-mcp-adapter, use its declared direct tools or mcp proxy and its discovery instructions; do not invent tool names.
Native read, grep, find, and shell tools remain useful for documentation, configuration, logs, unindexed or unsupported files, exact text or regex searches the index cannot answer, and exact current-source verification before edits.
If the tools or index are missing, stale, failing, or insufficient, use native tools to finish the task. Refresh the index when appropriate; do not repeatedly retry unavailable retrieval or treat an empty index result as proof that source is absent.`;

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
  // A failed retrieval releases native searches for this run. Retry indexed
  // retrieval on the next run; retain no per-file, per-call or session history.
  let retrievalFailed = false;
  pi.on("session_start", () => { retrievalFailed = false; });
  pi.on("tool_result", (event) => {
    if (event.isError && /^(?:mcp__tokensave__)?tokensave_(?:context|search|callers|callees|impact|read|files|body)$/.test(event.toolName)) {
      retrievalFailed = true;
    }
  });

  pi.on("tool_call", async (event, ctx) => {
    if (!GUARDED_TOOLS.has(event.toolName)) {
      return;
    }
    // Tool registration is not proof of transport health, but absence from
    // the active declarations means we cannot safely demand a direct call.
    // A generic mcp proxy is not evidence that Tokensave is configured. Keep
    // proxy-only and unknown/custom-prefix adapter setups advisory.
    const active = typeof pi.getActiveTools === "function" ? pi.getActiveTools() : [];
    const hasSearch = active.some(name => /^(?:mcp__tokensave__)?tokensave_search$/.test(name));
    const hasFiles = active.some(name => /^(?:mcp__tokensave__)?tokensave_files$/.test(name));
    if (retrievalFailed || !hasSearch || !hasFiles) {
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

  pi.on("before_agent_start", async (event) => {
    retrievalFailed = false;
    // Mutate the supported structured options rather than replacing Pi's prompt.
    // Older Pi versions retain the installed AGENTS.md rules and accounting.
    if (event.systemPromptOptions?.sections) {
      event.systemPromptOptions.sections.tokensave_retrieval = RETRIEVAL_GUIDANCE;
    }
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
