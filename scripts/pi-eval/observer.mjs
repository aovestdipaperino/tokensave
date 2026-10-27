// Refs #832. Observe normalized provider-bound state without changing it.
import { appendFileSync } from "node:fs";

export default function (pi) {
  let bytes = 0;
  pi.on("context_with_system", (event) => {
    const record = JSON.stringify({
      type: "provider_context",
      wall_time_ms: Date.now(),
      systemMessages: event.messages.filter(message => message.role === "system"),
    }) + "\n";
    bytes += Buffer.byteLength(record);
    if (bytes > 32 * 1024 * 1024) throw new Error("Evaluation prompt trace exceeded 32 MiB");
    appendFileSync(process.env.TOKENSAVE_PI_EVAL_CONTEXT, record, { mode: 0o600 });
  });
}
