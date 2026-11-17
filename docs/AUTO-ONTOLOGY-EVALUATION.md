# NVIDIA Auto Ontology interoperability evaluation

[Refs #1022](https://github.com/aovestdipaperino/tokensave/issues/1022)

**Decision.** Rely on agent-side MCP composition when an agent needs a live
catalog. For offline use, curate a small OKF bundle and import it with the
existing TokenSave commands. The PoC retrieved catalog definitions and linked
them to indexed code, but it did not establish a benefit over live composition
or ordinary, curated OKF. Defer a general adapter and a built-in connector.
There is no new runtime behavior or dependency.

Two use cases drove the evaluation: locating the code that implements a
business concept and bringing its separately maintained definition into a code
review. The same pattern could surface a catalog link for troubleshooting a
metric discrepancy. Use the catalog as the authority for business terminology
and TokenSave as the authority for indexed code locations.

## Compatibility

| Area | TokenSave | Auto Ontology and Ossie | Fidelity |
| --- | --- | --- | --- |
| Display and lookup | OKF concepts have a title, description, body, and tags. | Ossie entity concepts and datasets carry names and descriptions. The converter normalizes identifiers. | Copy description text verbatim. Keep the normalized concept and an explicit source identifier. Preserve the original display name separately if exact spelling matters. |
| Identity and provenance | OKF supports source references and verification metadata. This PoC stores a fixture digest and the qualified dataset name. | Auto Ontology owns certification and runtime identity. In the portable documents, entity identity is expressed through identifying attributes and dataset mappings. | Identifiers and revision text can be preserved as references. A digest identifies the snapshot. Certification, revocation, and access policy require a separate authoritative source. |
| Relationships | The code graph has symbol-level kinds, locations, and resolution evidence. OKF export summarizes relationships across files. | Ossie models joins, semantic relationships, roles, multiplicity, and mappings with database meaning. | A human can supply a code pointer. OKF links can cite related artifacts, while their graph semantics remain distinct. A relation with roles or cardinality cannot be reconstructed from a plain OKF link. |
| Query and execution | MCP retrieves indexed code and imported knowledge. OKF has a representation for attested computations. | SQL attributes, dialect-specific expressions, answerability, and generated SQL depend on the semantic model and source database. | SQL can be quoted as text. Executable semantics, join scope, correctness, and authorization stay with the catalog. |
| Interchange | OKF v0.2 is a Markdown bundle. TokenSave reads it without changing the code graph. | An Ossie export can contain a semantic model YAML or a ZIP with the model, ontology, and mapping YAML. The native vendor extension can carry a snapshot. | The ZIP carries more than display text. A private extension can retain native data for compatible readers. The Auto Ontology export path projects a multi-table term onto one dataset before conversion, and the OKF PoC does not consume private extensions. |

The comparison uses TokenSave's [OKF guide](guide/OKF.md), NVIDIA's pinned
[export implementation](https://github.com/NVIDIA/auto-ontology/blob/7dac5ff7bce3daf42e2115ef392146a542fa15d2/auto_ontology/server/model_interchange/service.py),
and the [Apache converter](https://github.com/apache/ossie-converters/tree/cb4b46b97bcdb9f2685ef2f7023b9134a4426da2/nvidia).

| Integration | Assessment |
| --- | --- |
| MCP composition | Register local TokenSave stdio and the authenticated Auto Ontology HTTP MCP endpoint in the same client. Route code location to TokenSave and glossary questions to Auto Ontology. This preserves catalog freshness and user permissions. It requires a compatible client, a reachable deployment, and separate catalog configuration. Reviewed against the pinned interface. Live sign-in and tool calls were not exercised. |
| Offline projection | Demonstrated with pinned converter output and local OKF import. It needs an explicit, reviewed concept-to-code map. Snapshots permit offline lookup but may go stale, and a local bundle has no per-term catalog ACL. |
| Explicit connector | The public gateway exposes authenticated REST and MCP surfaces. A connector would duplicate client routing and take on credential handling, retries, scope, and cross-server error reporting. The fixture gives no reason to add this to TokenSave. |

## Reproduce the offline PoC

The [fixture](../scripts/auto-ontology/fixture/) contains three fictional
billing concepts and one Python source file. Only the Python file is placed in
each temporary project. The harness uses the same two converter functions as
the NVIDIA ZIP export to produce the semantic model, ontology, and mapping in
memory. It derives three OKF concepts from those documents, adds explicit code
pointers and the fixture's SHA-256, then validates and imports the bundle
strictly. It drives a real TokenSave stdio MCP server in each arm. The fixture
uses single-table terms so NVIDIA's server-side multi-table projection is
unnecessary. It exercises the offline converter contract, with no NVIDIA HTTP
service, PostgreSQL, model endpoints, API keys, or actual billing data.

Use Linux, Python 3.11+, Git, and a stable TokenSave build with Python extraction
support. The setup below downloads packages and the tokenizer vocabulary. The
evaluation itself can run offline once those assets are present.

```sh
cargo build --locked --no-default-features --features lite --bin tokensave
python3 -m venv /tmp/tokensave-ontology-venv
. /tmp/tokensave-ontology-venv/bin/activate
python -m pip install 'tiktoken==0.12.0' 'PyYAML==6.0.3' 'sqlglot==30.33.0' \
  'git+https://github.com/apache/ossie-converters.git@cb4b46b97bcdb9f2685ef2f7023b9134a4426da2#subdirectory=nvidia'
export TIKTOKEN_CACHE_DIR=/tmp/tokensave-ontology-tokenizer
python -c 'import tiktoken; tiktoken.get_encoding("cl100k_base")'
python -B scripts/auto-ontology/evaluate.py --tokensave target/debug/tokensave
```

The script emits JSON. It creates fresh project and home directories per arm
and cleans them up when it exits. It aborts on tool errors, unexpected fixture
coverage, or unequal code-index sizes. The proof of concept is deliberately
limited to the included fixture and its hand-authored code links.

## Before and after

Linux x86-64, Intel Core i5-8350U, TokenSave 7.17.0 lite debug build from
`4ab595355287`, Rust 1.100.0. One initial pass and 20 warm repetitions per arm
with a single persistent stdio server for each arm.

| Task | Facts returned, baseline → OKF | MCP calls | Content tokens | Warm median tool time |
| --- | ---: | ---: | ---: | ---: |
| Locate the payment retry implementation | 1/1 → 1/1 | 1 → 1 | 24 → 24 | 10.95 → 10.44 ms |
| Locate invoice booking and explain net booked revenue | 1/3 → 3/3 | 2 → 3 | 30 → 392 | 23.30 → 25.60 ms |
| Locate subscription renewal and explain active subscriber | 1/3 → 3/3 | 2 → 3 | 30 → 372 | 23.57 → 25.51 ms |
| All tasks, first-pass evidence | 3/7 → 7/7 | 5 → 7 | 84 → 788 | — |

Mixed tasks score a code hit, the exact definition, and a retrieved link
between that definition and the indexed code file. Both arms look up the same
symbol. For the business question, the baseline searches source text for the
business term. The OKF arm searches the bundle and reads its top hit. Useful
facts per tool response were 0.60 → 1.00 across the suite. The first task only
uses code search in both arms.

Peak server RSS (`VmHWM`) was 45,484 → 46,444 KiB in this run. Project index
files totaled 516,096 bytes in both arms after server shutdown. The OKF arm
added 1,472 bytes of imported files, including the import manifest. The latency
column sums client-observed tool time per task and excludes setup, model time,
and agent deliberation. Content tokens count text returned by MCP using
`cl100k_base` and exclude tool schemas, prompts, envelopes, and answers. RSS is
the server process only and excludes converter and client memory.

The missing business facts were intentionally held in the extra catalog. The
results show the cost and benefit of making them retrievable on this fixture.
They do not measure agent routing, SQL correctness, live MCP latency or
inference spend, large-catalog storage, or benefit over a manually written OKF
bundle. The single host, debug build, and tiny sample constrain any performance
inference.

## Live setup and operational considerations

For a live trial, start the pinned Auto Ontology deployment with its web/auth
gateway, backend, ingestion service, and PostgreSQL with pgvector. Configure a
catalog and scoped account. SQL questions also need a queryable source
database and the configured reasoning, embedding, and other inference
endpoints used by that deployment. Run the separately packaged MCP server
against the web gateway and configure its public origin consistently on both
sides. In an MCP client that supports remote HTTP OAuth and stdio, add the
Ontology server's `/mcp` URL alongside the normal TokenSave entry. Sign in
through the client, test `search_terms` and `get_term`, and check deployment
readiness before attempting SQL. Setup was reviewed against the pinned
[deployment instructions](https://github.com/NVIDIA/auto-ontology/blob/7dac5ff7bce3daf42e2115ef392146a542fa15d2/README.md)
and [MCP contract](https://github.com/NVIDIA/auto-ontology/blob/7dac5ff7bce3daf42e2115ef392146a542fa15d2/docs/mcp.md).
The local Docker socket denied access, so no live benchmark was run.

The MCP client holds the user's OAuth token and Auto Ontology checks user
permissions at the gateway. The REST API also supports scoped user access via
API tokens with the issuing user's permissions. Keep the private backend and
ingestion endpoints behind the gateway. OAuth discovery, token expiry, a
missing live database, a gateway outage, and tool or schema changes all need
to be handled when using the live service. Keep the two client connections
independent so a failed catalog lookup can be reported separately from code
results. For reproducibility, pin and recheck the MCP tools and export shape
before changing the reviewed upstream revision. Auto Ontology and the
converter are Apache-2.0; TokenSave is MIT. The PoC invokes the separately
installed converter and ships only original fixture data. Review the
applicable license and data rights before redistributing a real catalog.

A live server receives the glossary query. Its text-to-SQL flow can send context
to configured inference providers. Use synthetic data for tests. Get explicit
opt-in before including local source, identifiers, or indexed metadata in any
external request. An exported catalog can itself contain internal names,
database mappings, and sensitive metadata, including fields in vendor
extensions. Review what is exported and where the reviewed projection will be
stored or shared. Imported OKF is a local copy with a separate lifecycle.
Remove or replace it when approval changes or the snapshot becomes stale.
