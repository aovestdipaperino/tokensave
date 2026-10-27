// Refs #832. Verify observation does not change provider-bound messages.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import observe from "../../scripts/pi-eval/observer.mjs";

test("observer records only system state and leaves messages unchanged", () => {
  const temporary = mkdtempSync(join(tmpdir(), "tokensave-pi-observer-"));
  const previous = process.env.TOKENSAVE_PI_EVAL_CONTEXT;
  try {
    process.env.TOKENSAVE_PI_EVAL_CONTEXT = join(temporary, "contexts.jsonl");
    let handler;
    observe({ on: (name, callback) => { assert.equal(name, "context_with_system"); handler = callback; } });
    const event = { messages: [
      { role: "system", content: "effective base", toolsAdded: [{ name: "read" }] },
      { role: "user", content: "private conversational input" },
    ] };
    const original = structuredClone(event);
    assert.equal(handler(event), undefined);
    assert.deepEqual(event, original);
    const row = JSON.parse(readFileSync(process.env.TOKENSAVE_PI_EVAL_CONTEXT, "utf8"));
    assert.deepEqual(row.systemMessages, [event.messages[0]]);
  } finally {
    if (previous === undefined) delete process.env.TOKENSAVE_PI_EVAL_CONTEXT;
    else process.env.TOKENSAVE_PI_EVAL_CONTEXT = previous;
    rmSync(temporary, { recursive: true });
  }
});
