// Copied by scripts/pi-tests/run.mjs into Pi's test/suite/<owned-temp-dir>.
// Refs tokensave #806 and earendil-works/pi #11410. No real provider calls.
import { fauxAssistantMessage, fauxToolCall, getCurrentSystemPrompt, getCurrentTools, type TranscriptContext } from "@earendil-works/pi-ai";
import { Type } from "typebox";
import { expect, test } from "vitest";
import { createHarness } from "../harness.ts";
import register from "./tokensave-extension.ts";

test("retrieval section reaches initial and subsequent provider requests", async () => {
  const harness = await createHarness({ extensionFactories: [register] });
  try {
    const prompts: string[] = [];
    harness.setResponses(["first", "second"].map(text => (context: TranscriptContext) => {
      prompts.push(getCurrentSystemPrompt(context.messages));
      expect(getCurrentTools(context.messages).map(tool => tool.name)).toContain("read");
      return fauxAssistantMessage(text);
    }));
    await harness.session.prompt("Find a symbol");
    await harness.session.prompt("Find its callers");
    expect(prompts).toHaveLength(2);
    for (const prompt of prompts) {
      expect(prompt.match(/<tokensave_retrieval>/g)).toHaveLength(1);
      expect(prompt).toContain("missing, stale, failing, or insufficient");
    }
  } finally { harness.cleanup(); }
});

test("real compaction preserves the section and the next run keeps native tools", async () => {
  const harness = await createHarness({
    settings: { compaction: { keepRecentTokens: 1 } },
    extensionFactories: [register, pi => {
      pi.on("session_before_compact", event => ({ compaction: {
        summary: "Deterministic offline summary", firstKeptEntryId: event.preparation.firstKeptEntryId,
        tokensBefore: event.preparation.tokensBefore,
      } }));
    }],
  });
  try {
    harness.setResponses([fauxAssistantMessage("first"), fauxAssistantMessage("second"), context => {
      expect(getCurrentSystemPrompt(context.messages)).toContain("<tokensave_retrieval>");
      expect(getCurrentTools(context.messages).map(tool => tool.name)).toContain("read");
      return fauxAssistantMessage("after compaction");
    }]);
    await harness.session.prompt("Explore source");
    await harness.session.prompt("Continue exploration");
    const result = await harness.session.compact();
    expect(result.summary).toBe("Deterministic offline summary");
    expect(harness.sessionManager.getEntries().some(entry => entry.type === "compaction")).toBe(true);
    expect(getCurrentSystemPrompt(harness.session.messages)).toContain("<tokensave_retrieval>");
    await harness.session.prompt("Verify current source");
    expect(harness.session.getLastAssistantText()).toBe("after compaction");
  } finally { harness.cleanup(); }
});

test.each(["mcp__tokensave__tokensave_search", "mcp"])("retrieval errors through %s still permit native reads", async name => {
  let reads = 0;
  const harness = await createHarness({ tools: [], extensionFactories: [register, pi => {
    pi.registerTool({ name, label: "Retrieval", description: "Simulated disconnected retrieval", parameters: Type.Object({}),
      execute: async () => { throw new Error("MCP disconnected (fixture)"); } });
    pi.registerTool({ name: "read", label: "Read", description: "Native source verification", parameters: Type.Object({}),
      execute: async () => { reads++; return { content: [{ type: "text", text: "current source" }], details: {} }; } });
  }] });
  try {
    harness.setResponses([
      fauxAssistantMessage(fauxToolCall(name, {}), { stopReason: "toolUse" }),
      fauxAssistantMessage(fauxToolCall("read", {}), { stopReason: "toolUse" }),
      fauxAssistantMessage("verified"),
    ]);
    await harness.session.prompt("Find source despite failed retrieval");
    expect(reads).toBe(1);
    expect(harness.session.messages.some(message => message.role === "toolResult" && message.toolName === name && message.isError)).toBe(true);
    expect(harness.session.getLastAssistantText()).toBe("verified");
  } finally { harness.cleanup(); }
});
