// Run with Node 22.6+: node --experimental-strip-types --test tests/pi_extension_test.mjs
// No Pi installation, model API, user configuration, or additional dependency.
import assert from "node:assert/strict";
import { test } from "node:test";
import register from "../src/agents/integrations/pi_extension.ts";

function extension() {
  const handlers = new Map();
  register({ on: (name, handler) => handlers.set(name, handler) });
  return handlers;
}

test("initial and subsequent runs contribute one section without replacing prompt state", async () => {
  const handler = extension().get("before_agent_start");
  for (const messageType of ["user", "user", "custom"]) {
    const options = {
      customPrompt: "User's custom prefix",
      selectedTools: ["read", "bash", "mcp"],
      promptGuidelines: ["User guideline"],
      sections: { mcp_servers: "Adapter-owned server inventory" },
    };
    const original = structuredClone(options);
    assert.equal(await handler({ messageType, systemPromptOptions: options }), undefined);
    const guidance = options.sections.tokensave_retrieval;
    assert.match(guidance, /prefer tokensave when its tools are reachable/);
    assert.match(guidance, /appropriate project index/);
    assert.match(guidance, /mcp__tokensave__<tool>/);
    assert.match(guidance, /pi-mcp-adapter/);
    assert.match(guidance, /declared direct tools or mcp proxy/);
    assert.match(guidance, /documentation, configuration, logs, unindexed/);
    assert.match(guidance, /exact current-source verification/);
    assert.match(guidance, /missing, stale, failing, or insufficient/);
    assert.match(guidance, /empty index result/);
    await handler({ messageType, systemPromptOptions: options });
    assert.equal(options.sections.tokensave_retrieval, guidance);
    delete options.sections.tokensave_retrieval;
    assert.deepEqual(options, original);
  }
});

test("guidance is rebuilt from fresh options without conversation or extension memory", async () => {
  // This simulates the event boundary after compaction; it does not run Pi's compactor.
  const before = { sections: {} };
  await extension().get("before_agent_start")({ systemPromptOptions: before });
  const after = { sections: {} };
  await extension().get("before_agent_start")({ systemPromptOptions: after });
  assert.deepEqual(after, before);
});

test("older event contracts and an unavailable accounting binary fail open", async () => {
  // The unrendered __TOKENSAVE_BIN__ cannot spawn; no installed binary is invoked.
  assert.equal(await extension().get("before_agent_start")({}), undefined);
});

test("native reads and indexed or adapter calls remain unintercepted", async () => {
  const handler = extension().get("tool_call");
  for (const path of ["README.md", "package.json", "excluded/example.rs", "src/example.rs"]) {
    const event = { toolName: "read", input: { path, offset: 10, limit: 5 } };
    const original = structuredClone(event);
    assert.equal(await handler(event, { cwd: "/tmp" }), undefined);
    assert.deepEqual(event, original);
  }
  for (const toolName of ["read", "mcp", "mcp__tokensave__tokensave_read", "tokensave_search", "edit", "write"]) {
    assert.equal(await handler({ toolName, input: { path: "src/example.rs" } }, { cwd: "/tmp" }), undefined);
  }
  // An unavailable hook binary also leaves existing guarded native tools usable.
  for (const toolName of ["bash", "grep", "find"]) {
    assert.equal(await handler({ toolName, input: {} }, { cwd: "/tmp" }), undefined);
  }
});
