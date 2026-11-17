//! Parsing of a single OKF markdown document (OKF v0.2 §4).

use serde_json::Value;

use super::yaml;
use super::yaml::YamlError;

/// Largest frontmatter block accepted, in bytes.
pub const MAX_FRONTMATTER_BYTES: usize = 256 * 1024;

/// The frontmatter and body of one markdown file.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// Parsed frontmatter, or `None` when the file has no frontmatter block.
    pub frontmatter: Option<Value>,
    /// Everything after the closing delimiter.
    pub body: String,
    /// 1-based line on which the body starts.
    pub body_line: usize,
}

/// Why a document's frontmatter could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontmatterError {
    /// An opening `---` without a closing one.
    Unterminated,
    /// The block is larger than [`MAX_FRONTMATTER_BYTES`].
    TooLarge,
    /// The YAML could not be loaded.
    Yaml(YamlError),
    /// The YAML loaded but is not a mapping.
    NotMapping,
}

impl std::fmt::Display for FrontmatterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unterminated => write!(f, "frontmatter block has no closing `---` line"),
            Self::TooLarge => write!(
                f,
                "frontmatter block exceeds {} KiB",
                MAX_FRONTMATTER_BYTES / 1024
            ),
            Self::Yaml(e) => write!(f, "{e}"),
            Self::NotMapping => write!(f, "frontmatter must be a YAML mapping"),
        }
    }
}

fn is_delimiter(line: &str) -> bool {
    line.trim_end() == "---"
}

/// Splits a markdown file into frontmatter and body.
///
/// A file whose first line is not `---` has no frontmatter. An empty block
/// parses as an empty mapping, matching the OKF reference implementation.
pub fn parse(text: &str) -> Result<Document, FrontmatterError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Ok(Document {
            frontmatter: None,
            body: String::new(),
            body_line: 1,
        });
    };
    if !is_delimiter(first) {
        return Ok(Document {
            frontmatter: None,
            body: text.to_string(),
            body_line: 1,
        });
    }
    let start = first.len();
    let mut offset = start;
    let mut line_number = 1;
    for line in lines {
        line_number += 1;
        if is_delimiter(line) {
            let block = &text[start..offset];
            if block.len() > MAX_FRONTMATTER_BYTES {
                return Err(FrontmatterError::TooLarge);
            }
            let value = yaml::load(block).map_err(FrontmatterError::Yaml)?;
            let value = match value {
                Value::Null => Value::Object(serde_json::Map::new()),
                Value::Object(_) => value,
                _ => return Err(FrontmatterError::NotMapping),
            };
            return Ok(Document {
                frontmatter: Some(value),
                body: text[offset + line.len()..].to_string(),
                body_line: line_number + 1,
            });
        }
        offset += line.len();
        if offset - start > MAX_FRONTMATTER_BYTES {
            return Err(FrontmatterError::TooLarge);
        }
    }
    Err(FrontmatterError::Unterminated)
}

/// A markdown link found in a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The link text.
    pub text: String,
    /// The raw destination, without angle brackets or title.
    pub target: String,
    /// 1-based line number in the file.
    pub line: usize,
}

/// A markdown ATX heading found in a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// Number of leading `#` characters.
    pub level: usize,
    /// Heading text without the markers.
    pub text: String,
    /// 1-based line number in the file.
    pub line: usize,
}

/// The structural parts of a body that OKF assigns meaning to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BodyOutline {
    /// Inline links and images are excluded; reference-style definitions are
    /// included.
    pub links: Vec<Link>,
    pub headings: Vec<Heading>,
    /// Footnote labels used as `[^label]` references, in first-use order.
    pub footnotes: Vec<String>,
}

/// Extracts links, headings and footnote references from a markdown body.
///
/// Fenced and indented code blocks and inline code spans are skipped. This
/// is a deliberately small scanner covering the `CommonMark` constructs OKF
/// bundles use, not a full `CommonMark` parser.
pub fn outline(body: &str, first_line: usize) -> BodyOutline {
    let mut out = BodyOutline::default();
    let mut fence: Option<(char, usize)> = None;
    for (index, raw) in body.lines().enumerate() {
        let line_number = first_line + index;
        let trimmed = raw.trim_start();
        let indent = raw.len() - trimmed.len();
        if let Some((marker, width)) = fence {
            let run = trimmed.chars().take_while(|c| *c == marker).count();
            if run >= width && trimmed[run * marker.len_utf8()..].trim().is_empty() {
                fence = None;
            }
            continue;
        }
        if indent <= 3 {
            if let Some(marker) = trimmed.chars().next().filter(|c| *c == '`' || *c == '~') {
                let width = trimmed.chars().take_while(|c| *c == marker).count();
                if width >= 3 {
                    fence = Some((marker, width));
                    continue;
                }
            }
        }
        if indent >= 4 || raw.starts_with('\t') {
            continue;
        }
        if let Some(heading) = heading(trimmed, line_number) {
            out.headings.push(heading);
        }
        if let Some(link) = reference_definition(trimmed, line_number) {
            out.links.push(link);
            continue;
        }
        scan_inline(trimmed, line_number, &mut out);
    }
    out
}

fn heading(line: &str, line_number: usize) -> Option<Heading> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim_end().to_string();
    Some(Heading {
        level,
        text,
        line: line_number,
    })
}

/// `[label]: destination` link reference definitions. Footnote definitions
/// (`[^label]: text`) are not links.
fn reference_definition(line: &str, line_number: usize) -> Option<Link> {
    let rest = line.strip_prefix('[')?;
    if rest.starts_with('^') {
        return None;
    }
    let close = rest.find("]:")?;
    let label = &rest[..close];
    let destination = rest[close + 2..].split_whitespace().next()?;
    let destination = destination
        .strip_prefix('<')
        .and_then(|d| d.strip_suffix('>'))
        .unwrap_or(destination);
    Some(Link {
        text: label.to_string(),
        target: destination.to_string(),
        line: line_number,
    })
}

fn scan_inline(line: &str, line_number: usize, out: &mut BodyOutline) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'`' => {
                let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
                let ticks = &line[i..i + run];
                i = line[i + run..]
                    .find(ticks)
                    .map_or(i + run, |end| i + run + end + run);
            }
            b'[' => {
                let is_image = i > 0 && bytes[i - 1] == b'!';
                if bytes.get(i + 1) == Some(&b'^') {
                    if let Some(end) = line[i + 2..].find(']') {
                        let label = &line[i + 2..i + 2 + end];
                        if !label.is_empty() && !out.footnotes.iter().any(|f| f == label) {
                            out.footnotes.push(label.to_string());
                        }
                        i += end + 3;
                        continue;
                    }
                }
                match inline_link(line, i) {
                    Some((text, target, end)) => {
                        if !is_image {
                            out.links.push(Link {
                                text,
                                target,
                                line: line_number,
                            });
                        }
                        i = end;
                    }
                    None => i += 1,
                }
            }
            _ => i += 1,
        }
    }
}

/// Parses `[text](destination "title")` starting at `start` (the `[`).
/// Returns the text, destination and the index just past the link.
fn inline_link(line: &str, start: usize) -> Option<(String, String, usize)> {
    let bytes = line.as_bytes();
    let mut depth = 0usize;
    let mut i = start;
    let text_end = loop {
        match bytes.get(i)? {
            b'\\' => i += 1,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    break i;
                }
            }
            _ => {}
        }
        i += 1;
    };
    if bytes.get(text_end + 1) != Some(&b'(') {
        return None;
    }
    let dest_start = text_end + 2;
    let rest = &line[dest_start..];
    let (target, consumed) = if let Some(inner) = rest.strip_prefix('<') {
        let close = inner.find('>')?;
        (inner[..close].to_string(), close + 2)
    } else {
        let mut parens = 0usize;
        let mut end = rest.len();
        for (offset, c) in rest.char_indices() {
            match c {
                '(' => parens += 1,
                ')' if parens == 0 => {
                    end = offset;
                    break;
                }
                ')' => parens -= 1,
                c if c.is_whitespace() => {
                    end = offset;
                    break;
                }
                _ => {}
            }
        }
        (rest[..end].to_string(), end)
    };
    let close = rest[consumed..].find(')')?;
    let text = line[start + 1..text_end].to_string();
    Some((text, target, dest_start + consumed + close + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_frontmatter_and_body() {
        let doc = parse("---\ntype: Metric\n---\n\n# Body\n").unwrap_or_else(|_| Document {
            frontmatter: None,
            body: String::new(),
            body_line: 0,
        });
        assert_eq!(doc.frontmatter.unwrap_or_default()["type"], "Metric");
        assert_eq!(doc.body, "\n# Body\n");
        assert_eq!(doc.body_line, 4);
    }

    #[test]
    fn handles_crlf_bom_and_empty_blocks() {
        let doc = parse("\u{feff}---\r\n---\r\nbody").ok();
        assert_eq!(doc.and_then(|d| d.frontmatter), Some(serde_json::json!({})));
    }

    #[test]
    fn reports_frontmatter_errors() {
        assert_eq!(parse("---\ntype: x\n"), Err(FrontmatterError::Unterminated));
        assert_eq!(parse("---\n- a\n---\n"), Err(FrontmatterError::NotMapping));
        assert!(matches!(
            parse("---\na: [\n---\n"),
            Err(FrontmatterError::Yaml(_))
        ));
    }

    #[test]
    fn file_without_frontmatter_is_all_body() {
        let doc = parse("# Title\n").ok();
        assert_eq!(doc.map(|d| d.frontmatter), Some(None));
    }

    #[test]
    fn outline_finds_links_headings_and_footnotes() {
        let body = "# Schema\n\
            See [orders](/tables/orders.md \"t\") and [x](<a b.md>).[^src]\n\
            ![img](pic.png) `[not](code.md)` \\[esc](no.md)\n\
            ```\n[fenced](skip.md)\n```\n\
            [ref]: ./ref.md\n\
            [^src]: Source\n\
            ## 2026-05-22\n";
        let out = outline(body, 10);
        let targets: Vec<&str> = out.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, ["/tables/orders.md", "a b.md", "./ref.md"]);
        assert_eq!(out.links[0].line, 11);
        assert_eq!(out.footnotes, ["src"]);
        let headings: Vec<(usize, &str)> = out
            .headings
            .iter()
            .map(|h| (h.level, h.text.as_str()))
            .collect();
        assert_eq!(headings, [(1, "Schema"), (2, "2026-05-22")]);
    }

    #[test]
    fn nested_brackets_and_parentheses() {
        let out = outline("[a [b] c](dir/x_(1).md)", 1);
        assert_eq!(out.links[0].text, "a [b] c");
        assert_eq!(out.links[0].target, "dir/x_(1).md");
    }
}
