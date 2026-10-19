//! Conservative fallback detection. Evidence levels are rules, not probabilities.
//! No parser probing, interpreter execution, or persistent side cache is used.
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::extraction::{LanguageExtractor, LanguageRegistry};

const DEFAULT_MAX_BYTES: usize = 4096;
const CACHE_LIMIT: usize = 4096;

/// Only unknown files are eligible; existing resolution always takes precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DetectionConfig {
    pub enabled: bool,
    #[serde(deserialize_with = "read_max_bytes")]
    pub max_bytes: usize,
    pub cache_results: bool,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_bytes: DEFAULT_MAX_BYTES,
            cache_results: true,
        }
    }
}

fn read_max_bytes<'de, D: serde::Deserializer<'de>>(d: D) -> Result<usize, D::Error> {
    let value = usize::deserialize(d)?;
    if !(128..=65536).contains(&value) {
        return Err(serde::de::Error::custom(
            "language_detection.max_bytes must be 128..=65536",
        ));
    }
    Ok(value)
}

impl DetectionConfig {
    fn valid(self) -> bool {
        (128..=65536).contains(&self.max_bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Shebang,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detection {
    pub language: &'static str,
    pub method: Method,
}

/// Process-local statistics; snapshots can be subtracted around a scan.
#[derive(Debug, Default, Clone, Copy)]
pub struct Statistics {
    pub examined: u64,
    pub shebang: u64,
    pub content: u64,
    pub unsupported: u64,
    pub cache_hits: u64,
    pub micros: u64,
}

impl std::ops::Sub for Statistics {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            examined: self.examined.saturating_sub(rhs.examined),
            shebang: self.shebang.saturating_sub(rhs.shebang),
            content: self.content.saturating_sub(rhs.content),
            unsupported: self.unsupported.saturating_sub(rhs.unsupported),
            cache_hits: self.cache_hits.saturating_sub(rhs.cache_hits),
            micros: self.micros.saturating_sub(rhs.micros),
        }
    }
}

#[derive(Hash, PartialEq, Eq)]
struct CacheKey {
    path: PathBuf,
    modified: Option<SystemTime>,
    changed: Option<SystemTime>,
    size: u64,
    config: DetectionConfig,
    // Feature availability is part of the key, including negative results.
    languages: String,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<CacheKey, Option<Detection>>,
    order: VecDeque<CacheKey>,
    stats: Statistics,
}

// At most 4096 decisions, including negatives. FIFO eviction releases paths and
// language fingerprints; no source bytes are retained and no tasks are spawned.
static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(Cache::default()));

pub fn statistics() -> Statistics {
    CACHE.lock().map(|c| c.stats).unwrap_or_default()
}

/// Reject sensitive formats before opening them, even if their text resembles code.
pub fn eligible(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    !matches!(
        ext.as_str(),
        "pem"
            | "key"
            | "crt"
            | "cer"
            | "der"
            | "p12"
            | "pfx"
            | "pub"
            | "p7b"
            | "p7c"
            | "keystore"
            | "jks"
    ) && !matches!(
        name.as_str(),
        "id_rsa" | "id_dsa" | "id_ecdsa" | "id_ed25519" | ".env"
    )
}

/// Bounded scan read. Metadata cache follows the same timestamp assumption as
/// incremental sync, at nanosecond precision and with Unix ctime as extra evidence.
/// A changed stat, settings, extractor set, or worktree path invalidates the key.
pub fn detect_file(
    registry: &LanguageRegistry,
    path: &Path,
    config: DetectionConfig,
) -> Option<Detection> {
    if !config.enabled || !config.valid() || !eligible(path) {
        return None;
    }
    let start = Instant::now();
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    let changed = {
        use std::os::unix::fs::MetadataExt;
        u64::try_from(metadata.ctime()).ok().and_then(|seconds| {
            SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::new(
                seconds,
                metadata.ctime_nsec() as u32,
            ))
        })
    };
    #[cfg(not(unix))]
    let changed = metadata.created().ok();
    let key = CacheKey {
        path: path.to_path_buf(),
        modified: metadata.modified().ok(),
        changed,
        size: metadata.len(),
        config,
        languages: registry
            .extractors()
            .iter()
            .map(|e| e.language_name())
            .collect::<Vec<_>>()
            .join("\0"),
    };
    if config.cache_results {
        if let Ok(mut cache) = CACHE.lock() {
            if let Some(result) = cache.entries.get(&key).copied() {
                cache.stats.cache_hits += 1;
                return result;
            }
        }
    }
    let mut bytes = Vec::with_capacity(config.max_bytes);
    std::fs::File::open(path)
        .ok()?
        .take(config.max_bytes as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    let result = detect(registry, &bytes, config);
    if let Ok(mut cache) = CACHE.lock() {
        cache.stats.examined += 1;
        cache.stats.micros += start.elapsed().as_micros() as u64;
        match result.map(|d| d.method) {
            Some(Method::Shebang) => cache.stats.shebang += 1,
            Some(Method::Content) => cache.stats.content += 1,
            None => cache.stats.unsupported += 1,
        }
        if config.cache_results && !cache.entries.contains_key(&key) {
            while cache.entries.len() >= CACHE_LIMIT {
                if let Some(old) = cache.order.pop_front() {
                    cache.entries.remove(&old);
                } else {
                    break;
                }
            }
            // The queue owns the key and the map owns an equivalent key; both
            // are bounded by CACHE_LIMIT. No source content is retained.
            let queued = CacheKey {
                path: key.path.clone(),
                languages: key.languages.clone(),
                ..key
            };
            cache.order.push_back(queued);
            cache.entries.insert(key, result);
        }
    }
    result
}

/// Detect from an already available source buffer, with no filesystem reads.
/// A partial final UTF-8 codepoint at the sampling boundary is ignored when
/// the caller supplies a longer buffer. Invalid samples fail conservatively.
pub fn detect(
    registry: &LanguageRegistry,
    bytes: &[u8],
    config: DetectionConfig,
) -> Option<Detection> {
    if !config.enabled || !config.valid() {
        return None;
    }
    let sample = &bytes[..bytes.len().min(config.max_bytes)];
    let text = match std::str::from_utf8(sample) {
        Ok(text) => text,
        Err(e) if e.error_len().is_none() && bytes.len() > config.max_bytes => {
            std::str::from_utf8(&sample[..e.valid_up_to()]).ok()?
        }
        Err(_) => return None,
    };
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        || text.contains("-----BEGIN ")
    {
        return None;
    }
    let (language, method) = if text.starts_with("#!") {
        // An unrecognized shebang is authoritative evidence against guessing
        // another language from the rest of the file.
        (shebang(text.lines().next()?)?, Method::Shebang)
    } else {
        (content_language(text)?, Method::Content)
    };
    extractor_for_detected_language(registry, language)?;
    Some(Detection { language, method })
}

fn shebang(line: &str) -> Option<&'static str> {
    let mut words = line.strip_prefix("#!")?.split_whitespace();
    let executable = words.next()?;
    if !executable.starts_with('/') {
        return None;
    }
    let mut name = executable.rsplit('/').next()?;
    if name == "env" {
        name = words.next()?;
        if name == "-S" {
            name = words.next()?;
        }
        // env flags, assignments and quoted commands are deliberately rejected.
    }
    match name {
        "python" | "python2" | "python3" => Some("Python"),
        n if n
            .strip_prefix("python3.")
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())) =>
        {
            Some("Python")
        }
        "bash" | "sh" | "dash" | "ksh" => Some("Bash"),
        "node" | "nodejs" => Some("JavaScript"),
        "ruby" => Some("Ruby"),
        _ => None,
    }
}

fn content_language(text: &str) -> Option<&'static str> {
    // Require two independent, characteristic signals. Generic words such as
    // `class`, `import`, or `function` alone cannot classify a file.
    let mut py_def = false;
    let mut py_body = false;
    let mut py_import = false;
    let mut rust_fn = false;
    let mut rust_marker = false;
    let mut js_fn = false;
    let mut js_module = false;
    let mut ts_type = false;
    let mut go_package = false;
    let mut go_fn = false;
    let mut ruby_def = false;
    let mut ruby_end = false;
    let mut c_include = false;
    let mut c_main = false;
    let mut cpp_marker = false;
    let mut block_comment = false;
    for line in text.lines() {
        let s = line.trim();
        if block_comment {
            block_comment = !s.contains("*/");
            continue;
        }
        if s.starts_with("/*") {
            block_comment = !s.contains("*/");
            continue;
        }
        if s.starts_with('#') && !s.starts_with("#include") {
            continue;
        }
        if s.starts_with("//") || s.starts_with("/*") || s.starts_with('*') {
            continue;
        }
        py_def |= (s.starts_with("def ") || s.starts_with("async def "))
            && s.contains('(')
            && s.ends_with(':');
        py_body |= line.starts_with("    ")
            && (s.starts_with("return ") || s == "pass" || s.starts_with("raise "));
        py_import |= s.starts_with("from ") && s.contains(" import ");
        rust_fn |=
            (s.starts_with("fn ") || s.starts_with("pub fn ") || s.starts_with("pub async fn "))
                && s.contains('(')
                && s.contains('{');
        rust_marker |= (s.starts_with("use ") && s.contains("::") && s.ends_with(';'))
            || s.starts_with("impl ") && s.ends_with('{')
            || s.starts_with("mod ") && s.ends_with(';')
            || s.contains("println!(");
        js_fn |= (s.starts_with("function ")
            || s.starts_with("export function ")
            || s.starts_with("async function "))
            && s.contains('(')
            && s.contains('{')
            || (s.starts_with("const ") && s.contains("=>"));
        js_module |= (s.starts_with("import ") || s.starts_with("export "))
            && (s.contains(" from ")
                || s.starts_with("export function ")
                || s.starts_with("export const "));
        ts_type |= (s.starts_with("interface ") || s.starts_with("export interface "))
            && s.contains('{')
            || (s.starts_with("type ") || s.starts_with("export type ")) && s.contains(" = ");
        go_package |=
            s.starts_with("package ") && !s.ends_with(';') && s.split_whitespace().count() == 2;
        go_fn |= s.starts_with("func ") && s.contains('(') && s.ends_with('{');
        ruby_def |= s.starts_with("def ") && !s.contains(':') && !s.contains('{');
        ruby_end |= s == "end";
        c_include |= s.starts_with("#include <") && s.ends_with('>');
        c_main |= (s.starts_with("int main(") || s.starts_with("int main (")) && s.contains('{');
        cpp_marker |= s.starts_with("using namespace ") && s.ends_with(';')
            || s.starts_with("std::")
            || s.starts_with("template<")
            || s.starts_with("template <");
    }
    let candidates = [
        ("Python", py_def && (py_body || py_import)),
        ("Rust", rust_fn && rust_marker),
        ("JavaScript", js_fn && js_module && !ts_type),
        ("TypeScript", ts_type && (js_fn || js_module)),
        ("Go", go_package && go_fn),
        ("Ruby", ruby_def && ruby_end),
        ("C", c_include && c_main && !cpp_marker),
        ("C++", c_include && cpp_marker),
    ];
    let mut matches = candidates.into_iter().filter(|(_, matched)| *matched);
    let (language, _) = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(language)
    }
}

/// Shared extraction fallback for in-process and isolated worker paths.
pub fn extractor_for_source<'r>(
    registry: &'r LanguageRegistry,
    root: &Path,
    path: &str,
    source: &str,
) -> Option<&'r dyn LanguageExtractor> {
    if !eligible(Path::new(path)) {
        return None;
    }
    // This is reached only for unknown paths; recognized files incur no config I/O.
    let config = settings_for(root)?;
    let result = detect(registry, source.as_bytes(), config)?;
    extractor_for_detected_language(registry, result.language)
}

fn extractor_for_detected_language<'r>(
    registry: &'r LanguageRegistry,
    language: &str,
) -> Option<&'r dyn LanguageExtractor> {
    // JavaScript shares the registered TypeScript extractor.
    registry.extractor_for_language(if language == "JavaScript" {
        "js"
    } else {
        language
    })
}

// Settings are cached separately because workers dispatch multiple files. At
// most 32 project roots are retained. File I/O occurs outside the lock.
type SettingsEntry = (PathBuf, Option<SystemTime>, u64, DetectionConfig);
static SETTINGS: LazyLock<Mutex<VecDeque<SettingsEntry>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

fn settings_for(root: &Path) -> Option<DetectionConfig> {
    let metadata = std::fs::metadata(crate::config::get_config_path(root)).ok();
    let modified = metadata.as_ref().and_then(|m| m.modified().ok());
    let size = metadata.as_ref().map_or(0, std::fs::Metadata::len);
    if let Ok(cache) = SETTINGS.lock() {
        if let Some(entry) = cache
            .iter()
            .find(|e| e.0 == root && e.1 == modified && e.2 == size)
        {
            return Some(entry.3);
        }
    }
    let config = crate::config::load_config(root).ok()?.language_detection;
    if let Ok(mut cache) = SETTINGS.lock() {
        cache.retain(|e| e.0 != root);
        if cache.len() == 32 {
            cache.pop_front();
        }
        cache.push_back((root.to_path_buf(), modified, size, config));
    }
    Some(config)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn decision_cache_has_a_fixed_bound_and_releases_evicted_paths() {
        let dir = tempfile::tempdir().unwrap();
        let registry = LanguageRegistry::new();
        let config = DetectionConfig {
            enabled: true,
            ..DetectionConfig::default()
        };
        for i in 0..=CACHE_LIMIT {
            let path = dir.path().join(format!("{i}.custom"));
            std::fs::write(&path, "unclassified text").unwrap();
            assert!(detect_file(&registry, &path, config).is_none());
        }
        let cache = CACHE.lock().unwrap();
        assert!(cache.entries.len() <= CACHE_LIMIT);
        assert_eq!(cache.entries.len(), cache.order.len());
        assert!(!cache
            .entries
            .keys()
            .any(|k| k.path == dir.path().join("0.custom")));
    }
}
