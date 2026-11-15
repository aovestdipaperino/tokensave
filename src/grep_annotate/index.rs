//! Read-only index lookups for the hits of a search command (#688).
//!
//! The index is opened read-only. This module starts no sync, no migration
//! and no write. Each lookup uses an existing index of the `files`, `nodes`,
//! `edges` or `ambiguous_calls` table.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use libsql::params;

use super::hit::bare_hit;
use super::hit::path_hit;
use super::plan::AnnotatePlan;
use crate::errors::Result;
use crate::tokensave::TokenSave;

/// Maximum number of files whose facts stay in memory.
const MAX_CACHED_FILES: usize = 2048;
/// Maximum number of symbols read for one file.
const MAX_SYMBOLS: i64 = 20_000;
/// Maximum number of edges read for one file.
const MAX_EDGES: i64 = 50_000;
/// Maximum number of call and reference facts on one hit.
const MAX_TARGET_FACTS: usize = 3;
/// Maximum number of candidate names on one ambiguous call.
const MAX_CANDIDATE_NAMES: usize = 3;

/// Node kinds that do not name the code around a hit.
const UNNAMED_KINDS: &[&str] = &[
    "file",
    "use",
    "annotation_usage",
    "decorator",
    "include",
    "export",
    "package",
    "library",
];

/// A symbol that a file defines.
struct Symbol {
    kind: String,
    name: String,
    display: String,
    start: u32,
    end: u32,
}

/// The target of an edge that starts on a line of a file.
struct Target {
    line: u32,
    is_call: bool,
    id: String,
    name: String,
    display: String,
    file: String,
    start: u32,
    end: u32,
}

/// A call that the resolver could not bind to one definition.
struct Ambiguous {
    line: u32,
    leaf: String,
    candidates: Vec<String>,
}

/// What the index knows about one file.
#[derive(Default)]
struct FileFacts {
    stale: bool,
    symbols: Vec<Symbol>,
    targets: Vec<Target>,
    ambiguous: Vec<Ambiguous>,
}

/// The result of [`Index::open`].
pub enum IndexState {
    /// The index is open.
    Ready(Box<Index>),
    /// The search is not in a project with an index.
    Missing,
    /// The project has an index that this command cannot read. The text says
    /// why, and what to do.
    Unusable(String),
}

/// A read-only view of the index of the project around a search.
pub struct Index {
    conn: libsql::Connection,
    /// Keeps the database open for `conn`.
    _database: libsql::Database,
    /// Canonical project root.
    root: PathBuf,
    /// Directory of the search, relative to `root`, with `/` separators.
    base: String,
    files: HashMap<String, Option<FileFacts>>,
    /// The last output path and the indexed file that it names.
    last_path: Option<(Vec<u8>, Option<String>)>,
    /// Whether the file of a definition is the same as when it was indexed.
    fresh_files: HashMap<String, bool>,
    /// Definitions whose line span is already in the output.
    located: HashSet<String>,
}

/// Removes the file path that the extractors put before a qualified name,
/// for example `src/a.rs::Parser::run` becomes `Parser::run`. The MCP tools
/// show names in the same form.
fn display_name(qualified_name: &str, file_path: &str) -> String {
    let prefix = format!("{file_path}::");
    let mut name = qualified_name;
    while let Some(rest) = name.strip_prefix(prefix.as_str()) {
        name = rest;
    }
    name.to_string()
}

/// Returns the length of the decoded text of a file.
///
/// A full index records this length as the size of the file. It is not the
/// size on disk for a file with a byte order mark, UTF-16 text or invalid
/// UTF-8, so the caller compares the two sizes.
fn decoded_size(path: &Path) -> Option<u64> {
    let source = crate::sync::read_source_file(path).ok()?;
    u64::try_from(source.len()).ok()
}

/// Formats a stored 0-based line span as 1-based editor lines.
fn span(start: u32, end: u32) -> String {
    if start == end {
        format!("{}", start + 1)
    } else {
        format!("{}–{}", start + 1, end + 1)
    }
}

fn stored_line(row: &libsql::Row, column: i32) -> Result<u32> {
    Ok(u32::try_from(row.get::<i64>(column)?).unwrap_or(0))
}

fn names_match(name: &str, plan: &AnnotatePlan) -> bool {
    plan.tokens.iter().any(|token| {
        if plan.ignore_case {
            token.to_lowercase() == name.to_lowercase()
        } else {
            token == name
        }
    })
}

/// Joins `path` to `base` and removes `.` and `..` components.
///
/// Returns `None` when the path leaves the project root.
fn join_normalized(base: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

impl Index {
    /// Opens the index of the project that owns `cwd`.
    ///
    /// When the result is not [`IndexState::Ready`], the caller prints the
    /// search output unchanged. `git_dirs` are the `git -C` directories of
    /// the command.
    pub async fn open(cwd: &Path, git_dirs: &[String]) -> IndexState {
        match Self::try_open(cwd, git_dirs).await {
            Ok(state) => state,
            Err(error) => IndexState::Unusable(error.to_string()),
        }
    }

    async fn try_open(cwd: &Path, git_dirs: &[String]) -> Result<IndexState> {
        // `git -C DIR` moves the search, so the project is the one that owns
        // that directory.
        let mut search_dir = cwd.to_path_buf();
        for dir in git_dirs {
            search_dir.push(dir);
        }
        let Some(root) = crate::config::discover_project_root(&search_dir) else {
            return Ok(IndexState::Missing);
        };
        // Serve the database that the MCP server serves for this checkout.
        let state_dir = crate::config::get_tokensave_dir(&root);
        let branch = crate::branch::current_branch(&root);
        let (path, _, _) = TokenSave::resolve_db_for_branch(&root, &state_dir, branch.as_deref());
        if !path.is_file() {
            return Ok(IndexState::Missing);
        }
        let latest = crate::db::migrations::latest_version();
        crate::db::compatibility::preflight(&path, latest).await?;
        // A direct connection skips the repository-wide caches that
        // `Database::open_read_only` fills.
        let database = libsql::Builder::new_local(path)
            .flags(libsql::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .build()
            .await?;
        let conn = database.connect()?;
        conn.execute_batch("PRAGMA query_only=1; PRAGMA busy_timeout=1000;")
            .await?;
        let mut versions = conn.query("PRAGMA user_version", ()).await?;
        let version = match versions.next().await? {
            Some(row) => row.get::<i64>(0)?,
            None => {
                return Ok(IndexState::Unusable(
                    "the index has no schema version".to_string(),
                ))
            }
        };
        drop(versions);
        // An older schema needs a migration, and this command never writes.
        if version != i64::from(latest) {
            return Ok(IndexState::Unusable(format!(
                "the index has schema version {version} and this build needs {latest}; \
                 run `tokensave sync` to update it"
            )));
        }

        let root = root.canonicalize()?;
        let search_dir = search_dir.canonicalize()?;
        let Ok(base) = search_dir.strip_prefix(&root) else {
            return Ok(IndexState::Missing);
        };
        let base = base.to_string_lossy().replace('\\', "/");
        Ok(IndexState::Ready(Box::new(Self {
            conn,
            _database: database,
            root,
            base,
            files: HashMap::new(),
            last_path: None,
            fresh_files: HashMap::new(),
            located: HashSet::new(),
        })))
    }

    /// Returns the facts to add to one line of search output, without the
    /// `#ts` marker. Returns `None` when the index has nothing for the line.
    ///
    /// `single_file` is the file of a search that prints `line:` hits with
    /// no path.
    pub async fn facts_for_line(
        &mut self,
        line: &[u8],
        plan: &AnnotatePlan,
        single_file: Option<&str>,
    ) -> Option<String> {
        if let Some((path, number)) = path_hit(line) {
            if let Some(file) = self.indexed_file(path).await {
                return self.describe(&file, number, plan).await;
            }
        }
        let file = single_file?;
        let number = bare_hit(line)?;
        let file = self.indexed_file(file.as_bytes()).await?;
        self.describe(&file, number, plan).await
    }

    /// Returns the project-relative path of the indexed file that an output
    /// path names.
    async fn indexed_file(&mut self, raw: &[u8]) -> Option<String> {
        if let Some((last, file)) = &self.last_path {
            if last == raw {
                return file.clone();
            }
        }
        let file = self.find_indexed_file(raw).await;
        self.last_path = Some((raw.to_vec(), file.clone()));
        file
    }

    async fn find_indexed_file(&mut self, raw: &[u8]) -> Option<String> {
        let raw = std::str::from_utf8(raw).ok()?.replace('\\', "/");
        let mut names = Vec::new();
        if Path::new(&raw).is_absolute() {
            let absolute = Path::new(&raw).canonicalize().ok()?;
            let relative = absolute.strip_prefix(&self.root).ok()?;
            names.push(relative.to_string_lossy().replace('\\', "/"));
        } else {
            names.extend(join_normalized(&self.base, &raw));
            // `git grep --full-name` prints paths relative to the repository
            // root, not to the directory of the search.
            if !self.base.is_empty() {
                names.extend(join_normalized("", &raw));
            }
        }
        for name in names {
            // A path that is not a file is the text of a context line or of
            // a log. It costs one `stat` and no database lookup.
            let may_be_indexed = self.files.contains_key(&name) || self.root.join(&name).is_file();
            if may_be_indexed && self.load(&name).await {
                return Some(name);
            }
        }
        None
    }

    /// Reads the facts of `file` into the cache. Returns whether the index
    /// has the file.
    async fn load(&mut self, file: &str) -> bool {
        if let Some(known) = self.files.get(file) {
            return known.is_some();
        }
        if self.files.len() >= MAX_CACHED_FILES {
            self.files.clear();
        }
        let facts = self.read_file_facts(file).await.ok().flatten();
        let found = facts.is_some();
        self.files.insert(file.to_string(), facts);
        found
    }

    /// Returns whether `file` has an index row and is the same as when it
    /// was indexed. `None` means that the index does not have the file.
    ///
    /// The file is changed when its modification time is later than the
    /// recorded time, or when its size is not the recorded size. The
    /// automatic sync tests the time only, so a file that was replaced by an
    /// older copy stays in the index until the next full sync.
    async fn file_state(&self, file: &str) -> Result<Option<bool>> {
        let mut rows = self
            .conn
            .query(
                "SELECT modified_at, size FROM files WHERE path = ?1",
                params![file],
            )
            .await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        let indexed_at: i64 = row.get(0)?;
        let indexed_size = u64::try_from(row.get::<i64>(1)?).unwrap_or(0);
        let path = self.root.join(file);
        let fresh = crate::sync::file_stat(&path).is_some_and(|(modified_at, size)| {
            modified_at <= indexed_at
                && (size == indexed_size || decoded_size(&path) == Some(indexed_size))
        });
        Ok(Some(fresh))
    }

    async fn read_file_facts(&self, file: &str) -> Result<Option<FileFacts>> {
        let Some(fresh) = self.file_state(file).await? else {
            return Ok(None);
        };
        if !fresh {
            return Ok(Some(FileFacts {
                stale: true,
                ..FileFacts::default()
            }));
        }
        let mut facts = FileFacts::default();

        let mut rows = self
            .conn
            .query(
                "SELECT kind, name, qualified_name, start_line, end_line
                 FROM nodes WHERE file_path = ?1 LIMIT ?2",
                params![file, MAX_SYMBOLS],
            )
            .await?;
        while let Some(row) = rows.next().await? {
            let kind: String = row.get(0)?;
            if UNNAMED_KINDS.contains(&kind.as_str()) {
                continue;
            }
            let qualified_name: String = row.get(2)?;
            facts.symbols.push(Symbol {
                kind,
                name: row.get(1)?,
                display: display_name(&qualified_name, file),
                start: stored_line(&row, 3)?,
                end: stored_line(&row, 4)?,
            });
        }
        drop(rows);

        let mut rows = self
            .conn
            .query(
                "SELECT e.line, e.kind, t.id, t.name, t.qualified_name, t.file_path,
                        t.start_line, t.end_line
                 FROM nodes s
                 JOIN edges e ON e.source = s.id
                 JOIN nodes t ON t.id = e.target
                 WHERE s.file_path = ?1 AND e.line IS NOT NULL AND e.kind != 'contains'
                 ORDER BY e.line, e.id
                 LIMIT ?2",
                params![file, MAX_EDGES],
            )
            .await?;
        while let Some(row) = rows.next().await? {
            let kind: String = row.get(1)?;
            let qualified_name: String = row.get(4)?;
            let target_file: String = row.get(5)?;
            facts.targets.push(Target {
                line: stored_line(&row, 0)?,
                is_call: kind == "calls",
                id: row.get(2)?,
                name: row.get(3)?,
                display: display_name(&qualified_name, &target_file),
                file: target_file,
                start: stored_line(&row, 6)?,
                end: stored_line(&row, 7)?,
            });
        }
        drop(rows);

        let mut rows = self
            .conn
            .query(
                "SELECT line, reference_name, candidate_node_ids
                 FROM ambiguous_calls WHERE file_path = ?1",
                params![file],
            )
            .await?;
        while let Some(row) = rows.next().await? {
            let reference_name: String = row.get(1)?;
            let encoded: String = row.get(2)?;
            facts.ambiguous.push(Ambiguous {
                line: stored_line(&row, 0)?,
                leaf: crate::resolution::simple_ref_name(&reference_name).to_string(),
                candidates: serde_json::from_str(&encoded).unwrap_or_default(),
            });
        }
        Ok(Some(facts))
    }

    /// Returns whether the file of a call target is the same as when it was
    /// indexed. The line span of a target in a changed file can be wrong.
    async fn is_fresh(&mut self, file: &str) -> bool {
        if let Some(fresh) = self.fresh_files.get(file) {
            return *fresh;
        }
        let fresh = self.file_state(file).await.ok().flatten().unwrap_or(false);
        self.fresh_files.insert(file.to_string(), fresh);
        fresh
    }

    /// Returns the display names of the first candidates of an ambiguous call.
    async fn candidate_names(&self, ids: &[String]) -> Vec<String> {
        let mut names = Vec::new();
        for id in ids.iter().take(MAX_CANDIDATE_NAMES) {
            let Ok(mut rows) = self
                .conn
                .query(
                    "SELECT qualified_name, file_path FROM nodes WHERE id = ?1",
                    params![id.as_str()],
                )
                .await
            else {
                continue;
            };
            if let Ok(Some(row)) = rows.next().await {
                if let (Ok(qualified_name), Ok(file)) = (row.get::<String>(0), row.get::<String>(1))
                {
                    names.push(display_name(&qualified_name, &file));
                }
            }
        }
        names
    }

    /// Builds the facts for the hit on 1-based line `number` of `file`.
    async fn describe(&mut self, file: &str, number: u32, plan: &AnnotatePlan) -> Option<String> {
        let line = number.checked_sub(1)?;
        let facts = self.files.get(file)?.as_ref()?;
        if facts.stale {
            return Some("stale".to_string());
        }
        let mut parts = Vec::new();

        // A symbol that starts on the hit line is a definition. A search for
        // its name prefers it. If there is none, the largest one wins: it
        // holds the others.
        let defined = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.start == line)
            .max_by_key(|symbol| {
                (
                    names_match(&symbol.name, plan),
                    symbol.end.saturating_sub(symbol.start),
                )
            });
        if let Some(symbol) = defined {
            parts.push(format!(
                "defines {} {} ({})",
                symbol.kind,
                symbol.display,
                span(symbol.start, symbol.end)
            ));
        } else if let Some(symbol) = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.start <= line && line <= symbol.end)
            .min_by_key(|symbol| (symbol.end.saturating_sub(symbol.start), symbol.start))
        {
            parts.push(format!(
                "in {} ({})",
                symbol.display,
                span(symbol.start, symbol.end)
            ));
        }

        // The command gives no match offsets. An edge on the hit line is
        // about the hit only when the search pattern names its target.
        let mut targets: Vec<&Target> = Vec::new();
        for target in &facts.targets {
            if target.line == line
                && names_match(&target.name, plan)
                && !targets.iter().any(|seen| seen.id == target.id)
            {
                targets.push(target);
            }
        }
        // Calls come first. The sort is stable, so line order stays.
        targets.sort_by_key(|target| !target.is_call);
        targets.truncate(MAX_TARGET_FACTS);
        let ambiguous: Vec<(String, Vec<String>)> = facts
            .ambiguous
            .iter()
            .filter(|call| call.line == line && names_match(&call.leaf, plan))
            .filter(|call| {
                !targets
                    .iter()
                    .any(|target| target.is_call && target.name == call.leaf)
            })
            .take(MAX_TARGET_FACTS.saturating_sub(targets.len()))
            .map(|call| (call.leaf.clone(), call.candidates.clone()))
            .collect();
        let targets: Vec<(bool, String, String, String, u32, u32)> = targets
            .into_iter()
            .map(|target| {
                (
                    target.is_call,
                    target.id.clone(),
                    target.display.clone(),
                    target.file.clone(),
                    target.start,
                    target.end,
                )
            })
            .collect();

        for (is_call, id, display, target_file, start, end) in targets {
            let relation = if is_call { "call" } else { "reference" };
            // The lines of a definition in a changed file can be wrong.
            if target_file != file && !self.is_fresh(&target_file).await {
                parts.push(format!("{relation} -> {display}"));
                continue;
            }
            // The first hit on a definition gives its line span. Later hits
            // give its first line, which is enough to find it again.
            let lines = if self.located.insert(id) {
                span(start, end)
            } else {
                span(start, start)
            };
            parts.push(format!("{relation} -> {display} ({target_file}:{lines})"));
        }
        for (_, candidates) in ambiguous {
            let names = self.candidate_names(&candidates).await;
            let more = if candidates.len() > names.len() {
                ", …"
            } else {
                ""
            };
            parts.push(format!(
                "call -> ? ({} candidates: {}{more})",
                candidates.len(),
                names.join(", ")
            ));
        }

        (!parts.is_empty()).then(|| parts.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_drops_the_file_prefix() {
        assert_eq!(
            display_name("src/a.rs::Parser::run", "src/a.rs"),
            "Parser::run"
        );
        assert_eq!(
            display_name("src/A.cs::src/A.cs::App::Run", "src/A.cs"),
            "App::Run"
        );
        assert_eq!(display_name("Parser::run", "src/a.rs"), "Parser::run");
    }

    #[test]
    fn span_shows_editor_lines() {
        assert_eq!(span(17, 39), "18–40");
        assert_eq!(span(4, 4), "5");
    }

    #[test]
    fn join_normalized_stays_in_the_project() {
        assert_eq!(join_normalized("", "src/a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(
            join_normalized("src/db", "./a.rs").as_deref(),
            Some("src/db/a.rs")
        );
        assert_eq!(
            join_normalized("src/db", "../a.rs").as_deref(),
            Some("src/a.rs")
        );
        assert_eq!(join_normalized("src", "../../a.rs"), None);
    }

    #[test]
    fn names_match_follows_the_case_rule() {
        let mut plan = AnnotatePlan {
            tokens: vec!["GetString".to_string()],
            ..AnnotatePlan::default()
        };
        assert!(names_match("GetString", &plan));
        assert!(!names_match("getstring", &plan));
        plan.ignore_case = true;
        assert!(names_match("getstring", &plan));
        assert!(!names_match("GetStrings", &plan));
    }
}
