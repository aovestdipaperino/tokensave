//! Reading of `path:line:` hits in the output of a search command (#688).

/// Maximum number of bytes at the start of a line that can hold the path and
/// the line number of a hit.
const MAX_HIT_PREFIX: usize = 4096;

/// Maximum number of digits in a line number.
const MAX_LINE_DIGITS: usize = 9;

/// Splits an output line into its text and its line terminator.
///
/// The terminator is `\n`, `\r\n` or empty.
pub fn split_terminator(line: &[u8]) -> (&[u8], &[u8]) {
    let text = line.strip_suffix(b"\n").unwrap_or(line);
    let text = text.strip_suffix(b"\r").unwrap_or(text);
    line.split_at(text.len())
}

/// Reads a line number that starts at `start` and ends with a colon.
fn line_number(line: &[u8], start: usize) -> Option<u32> {
    let digits = line
        .get(start..)?
        .iter()
        .take(MAX_LINE_DIGITS + 1)
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits == 0 || digits > MAX_LINE_DIGITS || line.get(start + digits) != Some(&b':') {
        return None;
    }
    std::str::from_utf8(line.get(start..start + digits)?)
        .ok()?
        .parse()
        .ok()
}

/// Reads an output line as `path:line:text`.
///
/// The path ends at the first `:digits:` in the line, so a drive letter stays
/// part of the path: `C:\a.rs:12:x` gives `C:\a.rs`. A context line such as
/// `a.rs-3-let s = "b:4:c"` also has this form. Its path is not a file, and
/// the caller checks that before it uses the hit.
pub fn path_hit(line: &[u8]) -> Option<(&[u8], u32)> {
    let prefix = line.get(..MAX_HIT_PREFIX).unwrap_or(line);
    prefix
        .iter()
        .enumerate()
        .filter(|(index, byte)| **byte == b':' && *index > 0)
        .find_map(|(index, _)| Some((line.get(..index)?, line_number(line, index + 1)?)))
}

/// Reads an output line as `line:text`, the form of a hit when the command
/// searches one file.
pub fn bare_hit(line: &[u8]) -> Option<u32> {
    line_number(line, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(line: &str) -> Option<(String, u32)> {
        path_hit(line.as_bytes())
            .map(|(path, number)| (String::from_utf8_lossy(path).into_owned(), number))
    }

    #[test]
    fn keeps_the_terminator_apart() {
        assert_eq!(split_terminator(b"a:1:x\n"), (&b"a:1:x"[..], &b"\n"[..]));
        assert_eq!(
            split_terminator(b"a:1:x\r\n"),
            (&b"a:1:x"[..], &b"\r\n"[..])
        );
        assert_eq!(split_terminator(b"a:1:x"), (&b"a:1:x"[..], &b""[..]));
        assert_eq!(split_terminator(b"\n"), (&b""[..], &b"\n"[..]));
    }

    #[test]
    fn reads_a_hit() {
        assert_eq!(
            hit("src/a.rs:12:    let x = 1;"),
            Some(("src/a.rs".to_string(), 12))
        );
        // ripgrep `--column` and grep `-b` put a second number after the line.
        assert_eq!(
            hit("src/a.rs:12:5:let x"),
            Some(("src/a.rs".to_string(), 12))
        );
    }

    #[test]
    fn reads_a_path_with_a_drive_letter() {
        assert_eq!(
            hit(r"C:\src\a.rs:7:fn main() {}"),
            Some((r"C:\src\a.rs".to_string(), 7))
        );
    }

    #[test]
    fn a_context_line_gives_no_path_of_its_own() {
        assert_eq!(hit("src/a.rs-12-    let x = 1;"), None);
        assert_eq!(hit("--"), None);
        // The text of this context line looks like a hit. Its path is not a
        // file, so the caller drops it.
        assert_eq!(
            hit(r#"src/a.rs-3-let s = "b:4:c";"#),
            Some((r#"src/a.rs-3-let s = "b"#.to_string(), 4))
        );
    }

    #[test]
    fn refuses_a_number_that_is_not_a_line() {
        assert_eq!(hit("a.rs:x:1"), None);
        assert_eq!(hit("a.rs:12"), None);
        assert_eq!(hit(":12:x"), None);
        assert_eq!(hit("a.rs:1234567890:x"), None);
    }

    #[test]
    fn reads_a_bare_hit() {
        assert_eq!(bare_hit(b"12:let x = 1;"), Some(12));
        assert_eq!(bare_hit(b"12-let x = 1;"), None);
        assert_eq!(bare_hit(b"let x = 1;"), None);
    }
}
