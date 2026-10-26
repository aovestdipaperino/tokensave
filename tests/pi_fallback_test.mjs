// node --experimental-strip-types --test tests/pi_fallback_test.mjs
import assert from "node:assert/strict";
import { test, mock } from "node:test";
import childProcess from "node:child_process";
import { syncBuiltinESMExports } from "node:module";
import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";
import register from "../src/agents/integrations/pi_extension.ts";

test("Pi guardrail requires active direct tools and releases searches after retrieval errors", async () => {
  let hookCalls = 0;
  mock.method(childProcess, "spawn", (_binary, args) => {
    const child = new EventEmitter();
    child.stderr = new PassThrough();
    child.stdin = new PassThrough();
    const guarded = args[0] === "hook-pi-pre-tool-use";
    if (guarded) hookCalls++;
    queueMicrotask(() => { child.stderr.end("Use tokensave_search"); child.emit("close", guarded ? 2 : 0); });
    return child;
  });
  syncBuiltinESMExports();
  try {
    const handlers = new Map();
    let active = [];
    register({ on: (name, handler) => handlers.set(name, handler), getActiveTools: () => active });
    const search = toolName => handlers.get("tool_call")({ toolName, input: { pattern: "symbol" } }, { cwd: "/tmp" });
    for (const names of [[], ["mcp"], ["mcp__other__tokensave_search"], ["mcp__tokensave__tokensave_search"], ["custom_tokensave_search", "custom_tokensave_files"]]) {
      active = names;
      assert.equal(await search("grep"), undefined);
    }
    assert.equal(hookCalls, 0);
    for (const prefix of ["mcp__tokensave__", ""]) {
      active = [`${prefix}tokensave_search`, `${prefix}tokensave_files`];
      for (const name of ["bash", "grep", "find"]) assert.equal((await search(name)).block, true);
      handlers.get("tool_result")({ toolName: `${prefix}tokensave_search`, isError: false });
      assert.equal((await search("grep")).block, true);
      handlers.get("tool_result")({ toolName: "mcp__other__tokensave_search", isError: true });
      assert.equal((await search("grep")).block, true);
      handlers.get("tool_result")({ toolName: `${prefix}tokensave_search`, isError: true });
      const before = hookCalls;
      for (const name of ["bash", "grep", "find", "read"]) assert.equal(await search(name), undefined);
      assert.equal(hookCalls, before);
      await handlers.get("before_agent_start")({ systemPromptOptions: { sections: {} } });
      assert.equal((await search("grep")).block, true);
      handlers.get("tool_result")({ toolName: `${prefix}tokensave_read`, isError: true });
      handlers.get("session_start")();
      assert.equal((await search("grep")).block, true);
    }
    active = [];
    assert.equal(await search("bash"), undefined);
  } finally {
    mock.restoreAll();
    syncBuiltinESMExports();
  }
});
