// Refs #832. Real Pi lifecycle with a faux provider; not real-model evidence.
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fauxAssistantMessage, getCurrentSystemPrompt, getCurrentTools } from "@earendil-works/pi-ai";
import { expect, test } from "vitest";
import { createHarness } from "../harness.ts";
import observe from "./evaluation-observer.mjs";
import register from "./tokensave-extension.ts";

test("evaluation observer captures actual normalized provider state", async () => {
  const temporary = mkdtempSync(join(tmpdir(), "tokensave-pi-observer-"));
  const previous = process.env.TOKENSAVE_PI_EVAL_CONTEXT;
  const output = join(temporary, "contexts.jsonl");
  process.env.TOKENSAVE_PI_EVAL_CONTEXT = output;
  const harness = await createHarness({ extensionFactories: [register, observe] });
  try {
    harness.setResponses([context => {
      const observed = JSON.parse(readFileSync(output, "utf8").trim());
      expect(getCurrentSystemPrompt(observed.systemMessages)).toBe(getCurrentSystemPrompt(context.messages));
      expect(getCurrentTools(observed.systemMessages)).toEqual(getCurrentTools(context.messages));
      expect(getCurrentSystemPrompt(observed.systemMessages)).toContain("<tokensave_retrieval>");
      return fauxAssistantMessage("observed");
    }]);
    await harness.session.prompt("Observe a request");
    expect(harness.session.getLastAssistantText()).toBe("observed");
  } finally {
    harness.cleanup();
    if (previous === undefined) delete process.env.TOKENSAVE_PI_EVAL_CONTEXT;
    else process.env.TOKENSAVE_PI_EVAL_CONTEXT = previous;
    rmSync(temporary, { recursive: true });
  }
});
