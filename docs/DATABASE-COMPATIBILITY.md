# Database channel compatibility

Stable and beta can have different schemas. Creation origin and schema version
are separate checks: neither a matching version nor an absent origin proves a
legacy beta database is stable-compatible.

## Storage and initialization

| State | Stable | Beta / prerelease |
| --- | --- | --- |
| Project database | `.tokensave/tokensave.db` | `.tokensave/beta/tokensave.db` |
| Project config, branch metadata, branch databases, locks | `.tokensave/` | `.tokensave/beta/` |
| Global database | `~/.tokensave/global.db` | `~/.tokensave/beta/global.db` |

With a base state directory set (`TOKENSAVE_STATE_DIR` or `state_dir`, see
[Keeping state outside the repository](guide/INDEXING.md#keeping-state-outside-the-repository)),
the project rows move to `<base>/projects/<id>/` (stable) and
`<base>/projects/<id>/beta/` (beta) with the same contents; the global rows do
not move. The base only changes where the files are: origin and schema checks
run on the database at its new path exactly as above, before any migration.

Global user settings and agent installation settings remain shared. The release
channel comes from the existing compiled release-channel selection,
not the Git branch being indexed or a runtime override.

Fresh databases record their creation channel in SQLite's existing
`application_id` header: `0x54535354` (stable) or `0x54534254` (beta). This adds no
graph-table migration or schema-version bump. Copies retain the original marker.
Opening, migrating, or reinitializing an existing database never changes origin.

Zero is legacy/unknown origin. Supported legacy schemas keep their migration
path; an unmarked database is not retroactively labeled. Older beta origin
cannot be reliably inferred when no marker exists. Unknown nonzero IDs are
rejected to avoid claiming a database owned by another application.

Stable rejects beta origin. Beta can explicitly open supported stable-origin
or legacy databases without relabeling them, but normal discovery uses isolated
beta paths and does not open stable databases. Both channels reject schemas
newer than the binary supports, in debug and release builds.

## Existing beta installations and returning to stable

After upgrading to isolated beta storage, run the beta binary's `init` in each
project and index it again. Reapply project config and manifests deliberately
inside `.tokensave/beta/` if needed. No database, branch metadata, savings history,
or configuration is automatically moved, copied, claimed, or deleted from the
old shared location. Keep backups of legacy beta state. A fresh beta global
database starts separate accounting history.

To return to stable, use the existing stable index or initialize a fresh stable
index. If the old shared location contains a beta database, preserve it in a
backup outside the stable paths before creating the stable index. Updated
stable builds refuse marked beta databases and unsupported newer schemas;
`sync --force` does not override that refusal, even after an interrupted run.

Older binaries ignore application IDs. Isolation prevents normal discovery of
new beta databases by older stable binaries, but cannot protect an explicitly
supplied beta path or an old beta database already in the stable location.
Do not point an older stable binary at beta state. Older stable `wipe` commands
can also delete the nested beta directory; preserve backups before using them.
Updated stable wipe preserves the beta subtree.

## WAL sidecar page-size check

SQLite before 3.54.0 can return uninitialized page-cache memory when it
replays a `-wal` file whose page size differs from the database's
(CVE-2026-119375, RUSTSEC-2026-0459). The SQLite bundled with libSQL 0.9.30
(3.45.1) does not have the fix. Before any connection opens a database,
`compatibility::preflight` reads the `-wal` sidecar, if one exists, and
refuses the database when SQLite would replay the WAL and:

- the WAL header page size differs from the database header page size,
- the database header is not a valid SQLite header, or
- a copy of page 1 in the WAL records a different page size, which SQLite
  would adopt after reading it.

SQLite writes WAL files at the database's page size, and the page size cannot
change in WAL mode, so a WAL written by SQLite never trips the check. A WAL
that is shorter than its header or has an invalid magic number or page size
passes, because SQLite ignores it. So does a WAL beside a missing or empty
database, which SQLite deletes on open.

A mismatch is a compatibility refusal: tokensave keeps both files and runs no
recovery. The message names the sidecar to delete before re-indexing. The
hook's read-only symbol probe applies the same check and fails closed. The
check reads two fixed-size headers and one page-number field per complete WAL
frame, plus the first 18 bytes of each page-1 frame, using constant memory.
It runs before SQLite opens the files and does not guard against a process
that rewrites the sidecar afterwards. Remove it when the bundled SQLite has
the upstream fix. Refs #940.

## Recovery and resources

Existing databases are checked with a short-lived read-only connection before
writable open and checked again on the writable connection before write
pragmas. Active WAL reads can use or create SQLite WAL/SHM sidecars; this is
not an immutable-file reader. Compatibility refusals do not run migration,
checkpoint, automatic corruption recovery, or forced reindex. Branch copying
also refuses incompatible sources before creating a destination.

Checks read fixed-size header values and a scalar schema count on creation;
there are no added caches, scans over graph rows, queues, or background tasks.
Connections and row cursors are scoped to each check and released on return.
The global accounting API retains its optional/best-effort contract and prints
a diagnostic before returning unavailable on an incompatible database.

Refs #835. Implementation and validation status are tracked in
[issue #835](https://github.com/aovestdipaperino/tokensave/issues/835).
