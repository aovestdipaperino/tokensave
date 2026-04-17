//! Global memory-mapped ring buffer for live token-savings monitoring.
//!
//! The mmap lives at `~/.tokensave/monitor.mmap` so a single TUI can
//! show activity from every project on the machine. Multiple MCP server
//! instances (one per project) write concurrently using file locking.
//!
//! Entry format is generic: each entry carries a **prefix** (tool suite
//! name, e.g. "tokensave"), a **project** (folder name), and a
//! **tool_name** (the specific MCP call).

use std::path::{Path, PathBuf};

// ── Layout constants ────────────────────────────────────────────────
const HEADER_SIZE: usize = 32;
const ENTRY_SIZE: usize = 128;
const RING_CAPACITY: usize = 256;
const FILE_SIZE: usize = HEADER_SIZE + ENTRY_SIZE * RING_CAPACITY;

const FIELD_LEN: usize = 32; // null-padded UTF-8 per string field

// Header offsets
const OFF_WRITE_IDX: usize = 0;
// bytes 8..32 reserved

// Entry field offsets (relative to entry start)
const EOFF_PREFIX: usize = 0;
const EOFF_PROJECT: usize = 32;
const EOFF_TOOL: usize = 64;
const EOFF_DELTA: usize = 96;
const EOFF_BEFORE: usize = 104;
const EOFF_TIMESTAMP: usize = 112;
// bytes 120..128 padding

const MMAP_FILENAME: &str = "monitor.mmap";
const LOCK_FILENAME: &str = "monitor.lock";

/// Resolve the global `~/.tokensave/` directory.
fn global_tokensave_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tokensave"))
}

/// A single ring-buffer entry read from the mmap.
#[derive(Debug, Clone)]
pub struct MonitorEntry {
    pub prefix: String,
    pub project: String,
    pub tool_name: String,
    pub delta: u64,
    pub before: u64,
    pub timestamp: u64,
}

impl MonitorEntry {
    /// Display label: `prefix - project - tool_name`
    pub fn label(&self) -> String {
        format!("{} - {} - {}", self.prefix, self.project, self.tool_name)
    }
}

// ── Writer (called by MCP server) ───────────────────────────────────

/// Write a tool-call entry to the global monitor mmap.
///
/// `project_root` is used to derive the folder name. `prefix` identifies
/// the tool suite (e.g. "tokensave"). Best-effort: silently returns on
/// any failure.
pub fn write_entry(
    project_root: &Path,
    prefix: &str,
    tool_name: &str,
    delta: u64,
    before: u64,
) {
    let Some(dir) = global_tokensave_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let mmap_path = dir.join(MMAP_FILENAME);
    let project = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let _ = write_entry_inner(&mmap_path, prefix, &project, tool_name, delta, before);
}

/// Write a tool-call entry to a specific mmap directory (for testing).
pub fn write_entry_to(
    dir: &Path,
    project_root: &Path,
    prefix: &str,
    tool_name: &str,
    delta: u64,
    before: u64,
) {
    let _ = std::fs::create_dir_all(dir);
    let mmap_path = dir.join(MMAP_FILENAME);
    let project = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let _ = write_entry_inner(&mmap_path, prefix, &project, tool_name, delta, before);
}

fn write_str(mmap: &mut memmap2::MmapMut, offset: usize, value: &str) {
    let bytes = value.as_bytes();
    let copy_len = bytes.len().min(FIELD_LEN - 1);
    mmap[offset..offset + FIELD_LEN].fill(0);
    mmap[offset..offset + copy_len].copy_from_slice(&bytes[..copy_len]);
}

fn write_entry_inner(
    mmap_path: &Path,
    prefix: &str,
    project: &str,
    tool_name: &str,
    delta: u64,
    before: u64,
) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(mmap_path)?;

    // Exclusive lock for concurrent writer safety.
    use fs2::FileExt;
    file.lock_exclusive()?;

    let len = file.metadata()?.len() as usize;
    if len < FILE_SIZE {
        file.set_len(FILE_SIZE as u64)?;
    }

    let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };

    // Read current write_idx.
    let write_idx = u64::from_le_bytes(
        mmap[OFF_WRITE_IDX..OFF_WRITE_IDX + 8]
            .try_into()
            .unwrap_or([0; 8]),
    );
    let slot = (write_idx as usize) % RING_CAPACITY;
    let off = HEADER_SIZE + slot * ENTRY_SIZE;

    // Write string fields.
    write_str(&mut mmap, off + EOFF_PREFIX, prefix);
    write_str(&mut mmap, off + EOFF_PROJECT, project);
    write_str(&mut mmap, off + EOFF_TOOL, tool_name);

    // Write numeric fields.
    mmap[off + EOFF_DELTA..off + EOFF_DELTA + 8].copy_from_slice(&delta.to_le_bytes());
    mmap[off + EOFF_BEFORE..off + EOFF_BEFORE + 8].copy_from_slice(&before.to_le_bytes());

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    mmap[off + EOFF_TIMESTAMP..off + EOFF_TIMESTAMP + 8]
        .copy_from_slice(&timestamp.to_le_bytes());

    // Increment write_idx (reader sees this last).
    let new_idx = write_idx + 1;
    mmap[OFF_WRITE_IDX..OFF_WRITE_IDX + 8].copy_from_slice(&new_idx.to_le_bytes());

    mmap.flush()?;
    file.unlock()?;
    Ok(())
}

// ── Reader (used by monitor TUI and tests) ──────────────────────────

/// Read-only view of the global monitor mmap.
pub struct MmapReader {
    mmap: memmap2::Mmap,
    dir: PathBuf,
}

fn read_str(mmap: &memmap2::Mmap, offset: usize) -> String {
    let bytes = &mmap[offset..offset + FIELD_LEN];
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(FIELD_LEN);
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

impl MmapReader {
    /// Open the global monitor mmap for reading.
    pub fn open() -> std::io::Result<Self> {
        let dir = global_tokensave_dir().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "cannot resolve home directory")
        })?;
        Self::open_at(&dir)
    }

    /// Open a monitor mmap at an explicit directory (for testing).
    pub fn open_at(dir: &Path) -> std::io::Result<Self> {
        let mmap_path = dir.join(MMAP_FILENAME);
        let file = std::fs::OpenOptions::new().read(true).open(&mmap_path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(Self { mmap, dir: dir.to_path_buf() })
    }

    /// Current write index (number of entries ever written).
    pub fn write_idx(&self) -> u64 {
        if self.mmap.len() < HEADER_SIZE {
            return 0;
        }
        u64::from_le_bytes(
            self.mmap[OFF_WRITE_IDX..OFF_WRITE_IDX + 8]
                .try_into()
                .unwrap_or([0; 8]),
        )
    }

    /// Read the entry at the given ring-buffer slot (0..255).
    pub fn entry(&self, slot: usize) -> Option<MonitorEntry> {
        if slot >= RING_CAPACITY {
            return None;
        }
        let off = HEADER_SIZE + slot * ENTRY_SIZE;
        if self.mmap.len() < off + ENTRY_SIZE {
            return None;
        }

        let prefix = read_str(&self.mmap, off + EOFF_PREFIX);
        let project = read_str(&self.mmap, off + EOFF_PROJECT);
        let tool_name = read_str(&self.mmap, off + EOFF_TOOL);

        let delta = u64::from_le_bytes(
            self.mmap[off + EOFF_DELTA..off + EOFF_DELTA + 8]
                .try_into()
                .unwrap_or([0; 8]),
        );
        let before = u64::from_le_bytes(
            self.mmap[off + EOFF_BEFORE..off + EOFF_BEFORE + 8]
                .try_into()
                .unwrap_or([0; 8]),
        );
        let timestamp = u64::from_le_bytes(
            self.mmap[off + EOFF_TIMESTAMP..off + EOFF_TIMESTAMP + 8]
                .try_into()
                .unwrap_or([0; 8]),
        );

        Some(MonitorEntry {
            prefix,
            project,
            tool_name,
            delta,
            before,
            timestamp,
        })
    }

    /// The ring buffer capacity.
    pub fn capacity(&self) -> usize {
        RING_CAPACITY
    }

    /// Re-read the mmap to pick up new writes.
    pub fn refresh(&mut self) -> std::io::Result<()> {
        let mmap_path = self.dir.join(MMAP_FILENAME);
        let file = std::fs::OpenOptions::new().read(true).open(&mmap_path)?;
        self.mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(())
    }
}

// ── TUI (tokensave monitor command) ─────────────────────────────────

use std::io::Write;

/// Run the monitor TUI. Blocks until Ctrl+C.
pub fn run() -> std::io::Result<()> {
    let dir = global_tokensave_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "cannot resolve home directory")
    })?;
    std::fs::create_dir_all(&dir)?;

    // Single-instance lock.
    let lock_path = dir.join(LOCK_FILENAME);
    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;

    use fs2::FileExt;
    if lock_file.try_lock_exclusive().is_err() {
        eprintln!("Monitor already running.");
        return Ok(());
    }

    // Ensure mmap file exists.
    let mmap_path = dir.join(MMAP_FILENAME);
    if !mmap_path.exists() {
        let f = std::fs::File::create(&mmap_path)?;
        f.set_len(FILE_SIZE as u64)?;
    }

    let mut reader = MmapReader::open()?;
    let mut last_idx = reader.write_idx();
    let mut entries: Vec<MonitorEntry> = Vec::new();

    // Populate with existing entries in the ring buffer (up to write_idx).
    let populated = last_idx.min(RING_CAPACITY as u64) as usize;
    if populated > 0 {
        let start_slot = if last_idx > RING_CAPACITY as u64 {
            (last_idx as usize) % RING_CAPACITY
        } else {
            0
        };
        for i in 0..populated {
            let slot = (start_slot + i) % RING_CAPACITY;
            if let Some(e) = reader.entry(slot) {
                if e.delta > 0 {
                    entries.push(e);
                }
            }
        }
    }

    // Enter raw mode + alternate screen.
    use crossterm::{
        cursor, execute, terminal,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen},
    };
    let mut stdout = std::io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let result = monitor_loop(&mut reader, &mut entries, &mut last_idx, &mut stdout);

    // Restore terminal.
    execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    let _ = lock_file.unlock();
    let _ = std::fs::remove_file(&lock_path);

    result
}

/// Cached cost data for the monitor panel, refreshed periodically.
struct CostCache {
    today_cost: f64,
    week_cost: f64,
    tokens_saved: u64,
    efficiency_pct: f64,
    top_model: String,
    top_model_cost: f64,
    last_refresh: std::time::Instant,
}

impl CostCache {
    fn new() -> Self {
        Self {
            today_cost: 0.0,
            week_cost: 0.0,
            tokens_saved: 0,
            efficiency_pct: 0.0,
            top_model: String::new(),
            top_model_cost: 0.0,
            last_refresh: std::time::Instant::now() - std::time::Duration::from_secs(999),
        }
    }

    fn is_stale(&self) -> bool {
        self.last_refresh.elapsed() > std::time::Duration::from_secs(30)
    }
}

/// Refresh cost data from the global DB. Best-effort, non-blocking.
/// Uses a tokio runtime because GlobalDb is async.
fn refresh_cost_cache(cache: &mut CostCache) {
    let future = async {
        let Some(gdb) = crate::global_db::GlobalDb::open().await else {
            return;
        };

        // Ingest any new data first
        crate::accounting::parser::ingest(&gdb).await;

        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let today_start = now_epoch - (now_epoch % 86400);
        let week_start = now_epoch.saturating_sub(7 * 86400);

        cache.today_cost = gdb.total_cost_since(today_start).await.unwrap_or(0.0);
        cache.week_cost = gdb.total_cost_since(week_start).await.unwrap_or(0.0);

        let week_consumed = gdb.total_tokens_since(week_start).await.unwrap_or(0);
        cache.tokens_saved = gdb.global_tokens_saved().await.unwrap_or(0);

        cache.efficiency_pct = if cache.tokens_saved + week_consumed > 0 {
            (cache.tokens_saved as f64 / (cache.tokens_saved + week_consumed) as f64) * 100.0
        } else {
            0.0
        };

        let models = gdb.cost_by_model_since(today_start).await;
        if let Some((model, cost, _)) = models.first() {
            cache.top_model = model.clone();
            cache.top_model_cost = *cost;
        }
    };
    // On Windows the monitor TUI runs inside #[tokio::main], so creating
    // a new runtime would panic. Use block_in_place + existing handle.
    #[cfg(windows)]
    {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(future)
        })
    }
    // On Unix the function may be called outside any tokio runtime,
    // so create a single-threaded one.
    #[cfg(not(windows))]
    {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        rt.block_on(future)
    }
    cache.last_refresh = std::time::Instant::now();
}

fn monitor_loop(
    reader: &mut MmapReader,
    entries: &mut Vec<MonitorEntry>,
    last_idx: &mut u64,
    stdout: &mut std::io::Stdout,
) -> std::io::Result<()> {
    use crossterm::{cursor, event, execute, terminal};

    let mut cost_cache = CostCache::new();

    loop {
        // Poll for key events (100ms timeout = our refresh rate).
        if event::poll(std::time::Duration::from_millis(100))? {
            if let event::Event::Key(key) = event::read()? {
                match key.code {
                    event::KeyCode::Char('c')
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                    {
                        break;
                    }
                    event::KeyCode::Char('r')
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                    {
                        entries.clear();
                    }
                    _ => {}
                }
            }
        }

        // Re-read mmap for new entries.
        let _ = reader.refresh();
        let current_idx = reader.write_idx();
        if current_idx > *last_idx {
            for i in *last_idx..current_idx {
                let slot = (i as usize) % RING_CAPACITY;
                if let Some(e) = reader.entry(slot) {
                    entries.push(e);
                }
            }
            *last_idx = current_idx;
        }

        // Refresh cost cache every 30 seconds.
        if cost_cache.is_stale() {
            refresh_cost_cache(&mut cost_cache);
        }

        // Render.
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let w = width as usize;
        let h = height as usize;

        execute!(stdout, cursor::MoveTo(0, 0))?;

        // Layout: cost panel (3 lines) + separator + log + separator + footer (2 lines)
        let has_cost = cost_cache.today_cost >= 0.001 || cost_cache.week_cost >= 0.001;
        let cost_lines = if has_cost { 4 } else { 0 }; // 3 lines + separator
        let footer_lines = 4; // separator + 2 footer lines + bottom separator
        let log_lines = h.saturating_sub(cost_lines + footer_lines).max(1);

        // ── Cost panel ──
        if has_cost {
            let sep = "\u{2500}".repeat(w);

            let saved_str = crate::display::format_token_count(cost_cache.tokens_saved);
            let line1 = format!(
                "  Spent: ${:.2} today | ${:.2} 7d    Saved: {}",
                cost_cache.today_cost, cost_cache.week_cost, saved_str
            );
            let line2 = format!(
                "  Efficiency: {:.0}%    Top model: {} (${:.2})",
                cost_cache.efficiency_pct, cost_cache.top_model, cost_cache.top_model_cost
            );

            write!(stdout, "\r\x1b[36m{}\x1b[0m{}\r\n", line1, " ".repeat(w.saturating_sub(line1.len())))?;
            write!(stdout, "\r\x1b[36m{}\x1b[0m{}\r\n", line2, " ".repeat(w.saturating_sub(line2.len())))?;
            write!(stdout, "\r{}\r\n", sep)?;
        }

        // ── Log entries (most recent at bottom of log area) ──
        let visible: Vec<&MonitorEntry> = entries
            .iter()
            .rev()
            .take(log_lines)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let blank_lines = log_lines.saturating_sub(visible.len());

        for _ in 0..blank_lines {
            write!(stdout, "\r{}\r\n", " ".repeat(w))?;
        }

        for entry in &visible {
            let label = entry.label();
            let delta_str = format_number(entry.delta);
            let padding = w.saturating_sub(label.len() + delta_str.len() + 2);
            write!(stdout, "\r{}{}{}\r\n", label, " ".repeat(padding), delta_str)?;
        }

        // ── Footer ──
        let sep = "\u{2500}".repeat(w);
        let total_saved: u64 = entries.iter().map(|e| e.delta).sum();
        let total_str = format_number(total_saved);
        let label = "TokenSave Monitor";
        let suffix = "saved tokens";
        let footer_content = format!("{}  {} {}", label, total_str, suffix);
        let footer_padding = w.saturating_sub(footer_content.len());
        let hint = "Ctrl+R reset | Ctrl+C quit";
        let hint_padding = w.saturating_sub(hint.len());

        write!(stdout, "\r{}\r\n", sep)?;
        write!(
            stdout,
            "\r{}{}\r\n",
            " ".repeat(footer_padding),
            footer_content
        )?;
        write!(stdout, "\r{}{}\r\n", " ".repeat(hint_padding), hint)?;
        write!(stdout, "\r{}", sep)?;

        stdout.flush()?;
    }
    Ok(())
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
