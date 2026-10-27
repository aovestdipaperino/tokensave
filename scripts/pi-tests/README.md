# Pi lifecycle regression tests

Run from Tokensave: `node scripts/pi-tests/run.mjs /path/to/prepared/pi`.

The supplied Pi checkout needs its pinned dependencies (`npm ci --ignore-scripts`)
and matching generated model data. Tested with Pi 1.4.0 at
`d5b3de22d0e24a3e9c38a9410d3329b099717283`. Follow Pi's model-data setup guide;
the matching published pi-ai package also includes `dist/providers/data`.
This runner neither downloads dependencies nor modifies Pi's tracked source.

The runner reserves an owned directory under Pi's test/suite, copies the actual
Tokensave extension and tests there, runs only those tests using Pi's Vitest
configuration, and removes the directory on completion or failure. It uses one
worker thread, forwards cancellation, and bounds execution to two minutes.
No real provider, API key, or paid model is used: Pi's faux provider drives
deterministic responses. MCP failure tests simulate tool errors; they do not
start a real MCP adapter or measure model routing quality.

Coverage includes initial/subsequent requests, actual session compaction using
an extension-supplied summary, prompt replay, native tool declarations, and
native reads after simulated direct/proxy retrieval errors.
It also verifies that the real-model evaluation observer captures the same
normalized prompt and tool state the faux provider receives, without mutation.
See [the real-model runner](../pi-eval/README.md) for paid evaluation scope;
these lifecycle fixtures remain offline tests.
