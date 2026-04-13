//! Shared memory-mapped ring buffer for live token-savings monitoring.
//!
//! The MCP server calls [`write_entry`] after each tool call. The
//! `tokensave monitor` TUI reads via [`MmapReader`]. Communication is
//! lock-free: the writer increments `write_idx` after filling a slot;
//! the reader polls `write_idx` to detect new entries.

use std::path::Path;

// ── Layout constants ────────────────────────────────────────────────
const VERSION: u8 = 1;
const HEADER_SIZE: usize = 32;
const ENTRY_SIZE: usize = 88;
const RING_CAPACITY: usize = 256;
const FILE_SIZE: usize = HEADER_SIZE + ENTRY_SIZE * RING_CAPACITY; // 22_560

const NAME_LEN: usize = 64; // null-padded UTF-8

// Header offsets
const OFF_VERSION: usize = 0;
const OFF_TOTAL_SAVED: usize = 1;
const OFF_WRITE_IDX: usize = 9;

// Entry field offsets (relative to entry start)
const EOFF_NAME: usize = 0;
const EOFF_DELTA: usize = 64;
const EOFF_BEFORE: usize = 72;
const EOFF_TIMESTAMP: usize = 80;

const MMAP_FILENAME: &str = "monitor.mmap";

/// A single ring-buffer entry read from the mmap.
#[derive(Debug, Clone)]
pub struct MonitorEntry {
    pub tool_name: String,
    pub delta: u64,
    pub before: u64,
    pub timestamp: u64,
}

// ── Writer (called by MCP server) ───────────────────────────────────

/// Write a tool-call entry to the monitor mmap. Best-effort: silently
/// returns on any failure (file missing, mmap error, etc.).
pub fn write_entry(project_root: &Path, tool_name: &str, delta: u64, before: u64) {
    let mmap_path = project_root.join(".tokensave").join(MMAP_FILENAME);
    let _ = write_entry_inner(&mmap_path, tool_name, delta, before);
}

fn write_entry_inner(
    mmap_path: &Path,
    tool_name: &str,
    delta: u64,
    before: u64,
) -> std::io::Result<()> {
    // Create or open the file, ensuring it is the right size.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(mmap_path)?;

    let len = file.metadata()?.len() as usize;
    if len < FILE_SIZE {
        file.set_len(FILE_SIZE as u64)?;
    }

    // Safety: we are the sole writer (MCP server is single-threaded for
    // tool dispatch), and the reader only reads. The worst case for a
    // torn read is a single garbled entry, which is acceptable.
    let mut mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };

    // Write version if fresh.
    if mmap[OFF_VERSION] != VERSION {
        mmap[OFF_VERSION] = VERSION;
    }

    // Read current write_idx.
    let write_idx = u64::from_le_bytes(
        mmap[OFF_WRITE_IDX..OFF_WRITE_IDX + 8]
            .try_into()
            .unwrap_or([0; 8]),
    );
    let slot = (write_idx as usize) % RING_CAPACITY;
    let entry_off = HEADER_SIZE + slot * ENTRY_SIZE;

    // Write tool name (null-padded).
    let name_bytes = tool_name.as_bytes();
    let copy_len = name_bytes.len().min(NAME_LEN - 1);
    mmap[entry_off + EOFF_NAME..entry_off + EOFF_NAME + NAME_LEN].fill(0);
    mmap[entry_off + EOFF_NAME..entry_off + EOFF_NAME + copy_len]
        .copy_from_slice(&name_bytes[..copy_len]);

    // Write delta, before, timestamp.
    mmap[entry_off + EOFF_DELTA..entry_off + EOFF_DELTA + 8]
        .copy_from_slice(&delta.to_le_bytes());
    mmap[entry_off + EOFF_BEFORE..entry_off + EOFF_BEFORE + 8]
        .copy_from_slice(&before.to_le_bytes());

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    mmap[entry_off + EOFF_TIMESTAMP..entry_off + EOFF_TIMESTAMP + 8]
        .copy_from_slice(&timestamp.to_le_bytes());

    // Update total_saved (add delta to current total).
    let current_total = u64::from_le_bytes(
        mmap[OFF_TOTAL_SAVED..OFF_TOTAL_SAVED + 8]
            .try_into()
            .unwrap_or([0; 8]),
    );
    let new_total = current_total + delta;
    mmap[OFF_TOTAL_SAVED..OFF_TOTAL_SAVED + 8].copy_from_slice(&new_total.to_le_bytes());

    // Increment write_idx (release fence — reader sees this last).
    let new_idx = write_idx + 1;
    mmap[OFF_WRITE_IDX..OFF_WRITE_IDX + 8].copy_from_slice(&new_idx.to_le_bytes());

    mmap.flush()?;
    Ok(())
}

// ── Reader (used by monitor TUI and tests) ──────────────────────────

/// Read-only view of the monitor mmap.
pub struct MmapReader {
    mmap: memmap2::Mmap,
}

impl MmapReader {
    /// Open an existing monitor mmap for reading.
    pub fn open(project_root: &Path) -> std::io::Result<Self> {
        let mmap_path = project_root.join(".tokensave").join(MMAP_FILENAME);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(&mmap_path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(Self { mmap })
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

    /// Cumulative total tokens saved.
    pub fn total_saved(&self) -> u64 {
        if self.mmap.len() < HEADER_SIZE {
            return 0;
        }
        u64::from_le_bytes(
            self.mmap[OFF_TOTAL_SAVED..OFF_TOTAL_SAVED + 8]
                .try_into()
                .unwrap_or([0; 8]),
        )
    }

    /// Read the entry at the given ring-buffer slot (0..255).
    pub fn entry(&self, slot: usize) -> Option<MonitorEntry> {
        if slot >= RING_CAPACITY {
            return None;
        }
        let entry_off = HEADER_SIZE + slot * ENTRY_SIZE;
        if self.mmap.len() < entry_off + ENTRY_SIZE {
            return None;
        }

        let name_bytes = &self.mmap[entry_off + EOFF_NAME..entry_off + EOFF_NAME + NAME_LEN];
        let name_end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(NAME_LEN);
        let tool_name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        let delta = u64::from_le_bytes(
            self.mmap[entry_off + EOFF_DELTA..entry_off + EOFF_DELTA + 8]
                .try_into()
                .unwrap_or([0; 8]),
        );
        let before = u64::from_le_bytes(
            self.mmap[entry_off + EOFF_BEFORE..entry_off + EOFF_BEFORE + 8]
                .try_into()
                .unwrap_or([0; 8]),
        );
        let timestamp = u64::from_le_bytes(
            self.mmap[entry_off + EOFF_TIMESTAMP..entry_off + EOFF_TIMESTAMP + 8]
                .try_into()
                .unwrap_or([0; 8]),
        );

        Some(MonitorEntry {
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

    /// Re-read the mmap to pick up new writes. On some OSes the kernel
    /// handles coherence automatically, but an explicit remap guarantees
    /// freshness on all platforms.
    pub fn refresh(&mut self, project_root: &Path) -> std::io::Result<()> {
        let mmap_path = project_root.join(".tokensave").join(MMAP_FILENAME);
        let file = std::fs::OpenOptions::new().read(true).open(&mmap_path)?;
        self.mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(())
    }
}

// ── TUI (tokensave monitor command) ─────────────────────────────────

use std::io::Write;

const LOCK_FILENAME: &str = "monitor.lock";

/// Run the monitor TUI. Blocks until Ctrl+C.
pub fn run(project_root: &Path) -> std::io::Result<()> {
    let tokensave_dir = project_root.join(".tokensave");
    if !tokensave_dir.is_dir() {
        eprintln!("No .tokensave directory found. Run `tokensave sync` first.");
        return Ok(());
    }

    // Single-instance lock.
    let lock_path = tokensave_dir.join(LOCK_FILENAME);
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
    let mmap_path = tokensave_dir.join(MMAP_FILENAME);
    if !mmap_path.exists() {
        let f = std::fs::File::create(&mmap_path)?;
        f.set_len(FILE_SIZE as u64)?;
    }

    let mut reader = MmapReader::open(project_root)?;
    let initial_total = reader.total_saved();
    let mut baseline = initial_total;
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

    let result = monitor_loop(
        project_root,
        &mut reader,
        &mut entries,
        &mut baseline,
        &mut last_idx,
        &mut stdout,
    );

    // Restore terminal.
    execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    // Release lock (dropped when lock_file goes out of scope, but explicit).
    let _ = lock_file.unlock();
    let _ = std::fs::remove_file(&lock_path);

    result
}

fn monitor_loop(
    project_root: &Path,
    reader: &mut MmapReader,
    entries: &mut Vec<MonitorEntry>,
    baseline: &mut u64,
    last_idx: &mut u64,
    stdout: &mut std::io::Stdout,
) -> std::io::Result<()> {
    use crossterm::{cursor, event, execute, terminal};

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
                        *baseline = reader.total_saved();
                        entries.clear();
                    }
                    _ => {}
                }
            }
        }

        // Re-read mmap for new entries.
        let _ = reader.refresh(project_root);
        let current_idx = reader.write_idx();
        if current_idx > *last_idx {
            // Read new entries.
            let start = *last_idx;
            let end = current_idx;
            for i in start..end {
                let slot = (i as usize) % RING_CAPACITY;
                if let Some(e) = reader.entry(slot) {
                    entries.push(e);
                }
            }
            *last_idx = current_idx;
        }

        // Render.
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let w = width as usize;
        let h = height as usize;

        execute!(stdout, cursor::MoveTo(0, 0))?;

        // Footer takes 3 lines. Log area = h - 3.
        let log_lines = if h > 4 { h - 4 } else { 1 };

        // Print log entries (most recent at bottom of log area).
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
            let name = &entry.tool_name;
            let delta_str = format_number(entry.delta);
            let padding = w.saturating_sub(name.len() + delta_str.len() + 2);
            write!(stdout, "\r{}{}{}\r\n", name, " ".repeat(padding), delta_str,)?;
        }

        // Footer.
        let sep = "\u{2500}".repeat(w);
        let total_saved = reader.total_saved().saturating_sub(*baseline);
        let total_str = format_number(total_saved);
        let label = "TokenSave Monitor";
        let suffix = "saved tokens";
        let footer_content = format!("{}  {} {}", label, total_str, suffix);
        let footer_padding = w.saturating_sub(footer_content.len());
        let hint = "Ctrl+R reset | Ctrl+C quit";
        let hint_padding = w.saturating_sub(hint.len());

        write!(stdout, "\r{}\r\n", sep)?;
        write!(stdout, "\r{}{}\r\n", " ".repeat(footer_padding), footer_content)?;
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
