// node --experimental-strip-types --test tests/pi_status_test.mjs
import assert from "node:assert/strict";
import { test } from "node:test";
import register from "../src/agents/integrations/pi_extension.ts";

test("live Pi status separates loaded prompt and declarations from unverified transport", async () => {
  const handlers = new Map();
  const commands = new Map();
  let active = [];
  register({ on: (name, handler) => handlers.set(name, handler), getActiveTools: () => active,
    registerCommand: (name, options) => commands.set(name, options) });
  let prompt = "";
  let message;
  const ctx = { getSystemPrompt: () => prompt, ui: { notify: text => { message = text; } } };
  const status = () => commands.get("tokensave-status").handler("", ctx);
  await status();
  assert.match(message, /extension: loaded/);
  assert.match(message, /current prompt: absent/);
  assert.match(message, /Native-search fallback: enabled/);
  assert.match(message, /health: not probed/);
  prompt = "<tokensave_retrieval>conditional preference</tokensave_retrieval>";
  active = ["mcp__tokensave__tokensave_search", "mcp__tokensave__tokensave_files"];
  await status();
  assert.match(message, /current prompt: present/);
  assert.match(message, /Native-search fallback: guardrail eligible/);
  handlers.get("tool_result")({ toolName: "mcp__tokensave__tokensave_search", isError: true });
  await status();
  assert.match(message, /Native-search fallback: enabled/);
  active = ["mcp"];
  await status();
  assert.match(message, /proxy: active; Tokensave configuration not verified/);
  assert.match(message, /direct tools: none/);
});
