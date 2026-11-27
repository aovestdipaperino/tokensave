//! Bounded decoder for the subset of the SCIP protobuf schema the comparison
//! prototype reads (#1100).
//!
//! Hand-written instead of generated so the experiment adds no dependency.
//! Field numbers follow `scip.proto` in sourcegraph/scip. Unknown fields are
//! skipped, so newer producers stay readable; group wire types, truncated
//! input, overlong varints, invalid UTF-8 and out-of-range integers are
//! refused. Every repeated field is counted against [`Limits`].

use std::fmt;

const DOCUMENT_TEXT_FIELD: u64 = 5;

/// `SymbolRole.Definition`.
pub const ROLE_DEFINITION: i32 = 0x1;
/// `SymbolRole.Import`.
pub const ROLE_IMPORT: i32 = 0x2;

/// Decoding budgets. The whole index is already in memory, so these bound the
/// decoded structures rather than the input itself.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_documents: usize,
    pub max_occurrences: usize,
    pub max_symbols: usize,
    pub max_string_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_documents: 200_000,
            max_occurrences: 20_000_000,
            max_symbols: 5_000_000,
            max_string_bytes: 64 * 1024,
        }
    }
}

/// Why an index could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(message: impl Into<String>) -> Result<T, DecodeError> {
    Err(DecodeError(message.into()))
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// `ProtocolVersion`; only `0` (`UnspecifiedProtocolVersion`) exists.
    pub version: u64,
    pub tool_name: String,
    pub tool_version: String,
    pub project_root: String,
    /// `TextEncoding`: 0 unspecified, 1 UTF-8, 2 UTF-16.
    pub text_encoding: u64,
}

/// A zero-based, half-open source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start_line: u32,
    pub start_char: u32,
    pub end_line: u32,
    pub end_char: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// `None` when the encoded range was not 3 or 4 non-negative integers.
    pub range: Option<Range>,
    pub symbol: String,
    pub roles: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub symbol: String,
    pub is_reference: bool,
    pub is_implementation: bool,
    pub is_type_definition: bool,
    pub is_definition: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SymbolInformation {
    pub symbol: String,
    pub relationships: Vec<Relationship>,
    pub kind: u64,
    pub display_name: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Document {
    pub relative_path: String,
    pub language: String,
    pub occurrences: Vec<Occurrence>,
    pub symbols: Vec<SymbolInformation>,
    /// Hash of the optional embedded source, retained in place of the text.
    pub text_hash: Option<String>,
    /// `PositionEncoding`: 0 unspecified, 1 UTF-8, 2 UTF-16, 3 UTF-32 code
    /// units from the line start.
    pub position_encoding: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Index {
    pub metadata: Metadata,
    pub documents: Vec<Document>,
    /// External symbols are counted but not decoded: the prototype maps only
    /// symbols defined inside the project.
    pub external_symbols: usize,
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Wire {
    Varint,
    Fixed64,
    Len,
    Fixed32,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn varint(&mut self) -> Result<u64, DecodeError> {
        let mut value: u64 = 0;
        for i in 0..10 {
            let Some(&byte) = self.buf.get(self.pos) else {
                return err("truncated varint");
            };
            self.pos += 1;
            if i == 9 && byte > 1 {
                return err("varint overflows 64 bits");
            }
            value |= u64::from(byte & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        err("varint longer than 10 bytes")
    }

    fn key(&mut self) -> Result<(u64, Wire), DecodeError> {
        let key = self.varint()?;
        let number = key >> 3;
        if number == 0 || number > 0x1FFF_FFFF {
            return err("invalid field number");
        }
        let wire = match key & 7 {
            0 => Wire::Varint,
            1 => Wire::Fixed64,
            2 => Wire::Len,
            5 => Wire::Fixed32,
            other => return err(format!("unsupported wire type {other}")),
        };
        Ok((number, wire))
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|&end| end <= self.buf.len())
            .ok_or_else(|| DecodeError("field length exceeds its message".into()))?;
        let bytes = &self.buf[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let len = usize::try_from(self.varint()?)
            .map_err(|_| DecodeError("field length exceeds address space".into()))?;
        self.take(len)
    }

    fn skip(&mut self, wire: Wire) -> Result<(), DecodeError> {
        match wire {
            Wire::Varint => self.varint().map(|_| ()),
            Wire::Fixed64 => self.take(8).map(|_| ()),
            Wire::Fixed32 => self.take(4).map(|_| ()),
            Wire::Len => self.bytes().map(|_| ()),
        }
    }
}

fn expect(wire: Wire, want: Wire, what: &str) -> Result<(), DecodeError> {
    if wire == want {
        Ok(())
    } else {
        err(format!("{what} has an unexpected wire type"))
    }
}

fn string(
    r: &mut Reader<'_>,
    wire: Wire,
    what: &str,
    limits: &Limits,
) -> Result<String, DecodeError> {
    expect(wire, Wire::Len, what)?;
    let bytes = r.bytes()?;
    if bytes.len() > limits.max_string_bytes {
        return err(format!("{what} exceeds {} bytes", limits.max_string_bytes));
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| DecodeError(format!("{what} is not UTF-8")))
}

fn uint(r: &mut Reader<'_>, wire: Wire, what: &str) -> Result<u64, DecodeError> {
    expect(wire, Wire::Varint, what)?;
    r.varint()
}

/// Decodes a protobuf `int32`, whose negative values occupy ten bytes.
fn int32(value: u64) -> Result<i32, DecodeError> {
    #[allow(clippy::cast_possible_wrap)]
    let signed = value as i64;
    i32::try_from(signed).map_err(|_| DecodeError("int32 out of range".into()))
}

struct Budget {
    documents: usize,
    occurrences: usize,
    symbols: usize,
}

impl Budget {
    fn spend(slot: &mut usize, limit: usize, what: &str) -> Result<(), DecodeError> {
        *slot += 1;
        if *slot > limit {
            return err(format!("index exceeds the {what} limit of {limit}"));
        }
        Ok(())
    }
}

/// Decodes a SCIP `Index`. Concatenated `Index` messages (SCIP's streaming
/// form) merge as protobuf specifies: documents append, metadata fields
/// overwrite.
pub fn decode_index(buf: &[u8], limits: &Limits) -> Result<Index, DecodeError> {
    let mut r = Reader::new(buf);
    let mut index = Index::default();
    let mut budget = Budget {
        documents: 0,
        occurrences: 0,
        symbols: 0,
    };
    while !r.done() {
        let (number, wire) = r.key()?;
        match number {
            1 => {
                expect(wire, Wire::Len, "Index.metadata")?;
                decode_metadata(r.bytes()?, &mut index.metadata, limits)?;
            }
            2 => {
                expect(wire, Wire::Len, "Index.documents")?;
                Budget::spend(&mut budget.documents, limits.max_documents, "document")?;
                index
                    .documents
                    .push(decode_document(r.bytes()?, limits, &mut budget)?);
            }
            3 => {
                expect(wire, Wire::Len, "Index.external_symbols")?;
                r.bytes()?;
                Budget::spend(&mut budget.symbols, limits.max_symbols, "symbol")?;
                index.external_symbols += 1;
            }
            _ => r.skip(wire)?,
        }
    }
    Ok(index)
}

fn decode_metadata(buf: &[u8], meta: &mut Metadata, limits: &Limits) -> Result<(), DecodeError> {
    let mut r = Reader::new(buf);
    while !r.done() {
        let (number, wire) = r.key()?;
        match number {
            1 => meta.version = uint(&mut r, wire, "Metadata.version")?,
            2 => {
                expect(wire, Wire::Len, "Metadata.tool_info")?;
                let mut t = Reader::new(r.bytes()?);
                while !t.done() {
                    let (n, w) = t.key()?;
                    match n {
                        1 => meta.tool_name = string(&mut t, w, "ToolInfo.name", limits)?,
                        2 => meta.tool_version = string(&mut t, w, "ToolInfo.version", limits)?,
                        // Arguments may hold local paths; never retained.
                        _ => t.skip(w)?,
                    }
                }
            }
            3 => meta.project_root = string(&mut r, wire, "Metadata.project_root", limits)?,
            4 => meta.text_encoding = uint(&mut r, wire, "Metadata.text_document_encoding")?,
            _ => r.skip(wire)?,
        }
    }
    Ok(())
}

fn decode_document(
    buf: &[u8],
    limits: &Limits,
    budget: &mut Budget,
) -> Result<Document, DecodeError> {
    let mut r = Reader::new(buf);
    let mut doc = Document::default();
    while !r.done() {
        let (number, wire) = r.key()?;
        match number {
            1 => doc.relative_path = string(&mut r, wire, "Document.relative_path", limits)?,
            2 => {
                expect(wire, Wire::Len, "Document.occurrences")?;
                Budget::spend(
                    &mut budget.occurrences,
                    limits.max_occurrences,
                    "occurrence",
                )?;
                doc.occurrences.push(decode_occurrence(r.bytes()?, limits)?);
            }
            3 => {
                expect(wire, Wire::Len, "Document.symbols")?;
                Budget::spend(&mut budget.symbols, limits.max_symbols, "symbol")?;
                doc.symbols.push(decode_symbol(r.bytes()?, limits)?);
            }
            4 => doc.language = string(&mut r, wire, "Document.language", limits)?,
            DOCUMENT_TEXT_FIELD => {
                expect(wire, Wire::Len, "Document.text")?;
                let text = std::str::from_utf8(r.bytes()?)
                    .map_err(|_| DecodeError("invalid UTF-8 in Document.text".into()))?;
                doc.text_hash = Some(crate::sync::content_hash(text));
            }
            6 => doc.position_encoding = uint(&mut r, wire, "Document.position_encoding")?,
            _ => r.skip(wire)?,
        }
    }
    Ok(doc)
}

fn decode_occurrence(buf: &[u8], limits: &Limits) -> Result<Occurrence, DecodeError> {
    let mut r = Reader::new(buf);
    let mut raw: Vec<i32> = Vec::with_capacity(4);
    let mut symbol = String::new();
    let mut roles = 0;
    while !r.done() {
        let (number, wire) = r.key()?;
        match number {
            1 => match wire {
                // proto3 packs repeated scalars, but unpacked input is legal.
                Wire::Len => {
                    let mut p = Reader::new(r.bytes()?);
                    while !p.done() {
                        if raw.len() == 4 {
                            return err("Occurrence.range has more than 4 elements");
                        }
                        raw.push(int32(p.varint()?)?);
                    }
                }
                Wire::Varint => {
                    if raw.len() == 4 {
                        return err("Occurrence.range has more than 4 elements");
                    }
                    raw.push(int32(r.varint()?)?);
                }
                _ => return err("Occurrence.range has an unexpected wire type"),
            },
            2 => symbol = string(&mut r, wire, "Occurrence.symbol", limits)?,
            3 => roles = int32(uint(&mut r, wire, "Occurrence.symbol_roles")?)?,
            _ => r.skip(wire)?,
        }
    }
    Ok(Occurrence {
        range: range(&raw),
        symbol,
        roles,
    })
}

fn range(raw: &[i32]) -> Option<Range> {
    let v: Vec<u32> = raw
        .iter()
        .map(|&x| u32::try_from(x).ok())
        .collect::<Option<_>>()?;
    match *v.as_slice() {
        [line, start, end] if end >= start => Some(Range {
            start_line: line,
            start_char: start,
            end_line: line,
            end_char: end,
        }),
        [start_line, start_char, end_line, end_char]
            if (end_line, end_char) >= (start_line, start_char) =>
        {
            Some(Range {
                start_line,
                start_char,
                end_line,
                end_char,
            })
        }
        _ => None,
    }
}

fn decode_symbol(buf: &[u8], limits: &Limits) -> Result<SymbolInformation, DecodeError> {
    let mut r = Reader::new(buf);
    let mut info = SymbolInformation::default();
    while !r.done() {
        let (number, wire) = r.key()?;
        match number {
            1 => info.symbol = string(&mut r, wire, "SymbolInformation.symbol", limits)?,
            4 => {
                expect(wire, Wire::Len, "SymbolInformation.relationships")?;
                if info.relationships.len() >= 4096 {
                    return err("symbol has more than 4096 relationships");
                }
                info.relationships
                    .push(decode_relationship(r.bytes()?, limits)?);
            }
            5 => info.kind = uint(&mut r, wire, "SymbolInformation.kind")?,
            6 => {
                info.display_name = string(&mut r, wire, "SymbolInformation.display_name", limits)?;
            }
            // Documentation and signatures are not needed and never retained.
            _ => r.skip(wire)?,
        }
    }
    Ok(info)
}

fn decode_relationship(buf: &[u8], limits: &Limits) -> Result<Relationship, DecodeError> {
    let mut r = Reader::new(buf);
    let mut rel = Relationship::default();
    while !r.done() {
        let (number, wire) = r.key()?;
        match number {
            1 => rel.symbol = string(&mut r, wire, "Relationship.symbol", limits)?,
            2 => rel.is_reference = uint(&mut r, wire, "Relationship.is_reference")? != 0,
            3 => {
                rel.is_implementation = uint(&mut r, wire, "Relationship.is_implementation")? != 0;
            }
            4 => {
                rel.is_type_definition =
                    uint(&mut r, wire, "Relationship.is_type_definition")? != 0;
            }
            5 => rel.is_definition = uint(&mut r, wire, "Relationship.is_definition")? != 0,
            _ => r.skip(wire)?,
        }
    }
    Ok(rel)
}

/// Minimal encoder used by tests to build indexes field by field.
#[cfg(test)]
pub(crate) mod encode {
    pub fn varint(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }
    pub fn len(number: u64, payload: &[u8], out: &mut Vec<u8>) {
        varint(number << 3 | 2, out);
        varint(payload.len() as u64, out);
        out.extend_from_slice(payload);
    }
    pub fn uint(number: u64, value: u64, out: &mut Vec<u8>) {
        varint(number << 3, out);
        varint(value, out);
    }
    pub fn occurrence(range: &[u32], symbol: &str, roles: u64) -> Vec<u8> {
        let mut packed = Vec::new();
        for &v in range {
            varint(u64::from(v), &mut packed);
        }
        let mut out = Vec::new();
        len(1, &packed, &mut out);
        len(2, symbol.as_bytes(), &mut out);
        if roles != 0 {
            uint(3, roles, &mut out);
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::encode::{len, occurrence, uint};
    use super::{decode_index, Limits, Range, DOCUMENT_TEXT_FIELD, ROLE_DEFINITION};

    fn index_with(doc: &[u8]) -> Vec<u8> {
        let mut meta = Vec::new();
        len(3, b"file:///p", &mut meta);
        let mut out = Vec::new();
        len(1, &meta, &mut out);
        len(2, doc, &mut out);
        out
    }

    #[test]
    fn decodes_documents_occurrences_and_relationships() {
        let mut rel = Vec::new();
        len(1, b"t . . . Trait#", &mut rel);
        uint(3, 1, &mut rel);
        let mut sym = Vec::new();
        len(1, b"t . . . Impl#", &mut sym);
        len(4, &rel, &mut sym);
        let mut doc = Vec::new();
        len(1, b"src/a.rs", &mut doc);
        len(2, &occurrence(&[1, 2, 5], "t . . . f().", 1), &mut doc);
        len(2, &occurrence(&[3, 0, 4, 1], "local 1", 0), &mut doc);
        len(3, &sym, &mut doc);
        let embedded_source = "fn f() {}";
        len(DOCUMENT_TEXT_FIELD, embedded_source.as_bytes(), &mut doc);
        len(99, b"unknown field is skipped", &mut doc);
        let index = decode_index(&index_with(&doc), &Limits::default()).unwrap();
        assert_eq!(index.metadata.project_root, "file:///p");
        let d = &index.documents[0];
        assert_eq!(d.relative_path, "src/a.rs");
        assert_eq!(
            d.text_hash,
            Some(crate::sync::content_hash(embedded_source))
        );
        assert_eq!(
            d.occurrences[0].range,
            Some(Range {
                start_line: 1,
                start_char: 2,
                end_line: 1,
                end_char: 5
            })
        );
        assert_eq!(d.occurrences[0].roles, ROLE_DEFINITION);
        assert_eq!(d.occurrences[1].range.unwrap().end_line, 4);
        assert!(d.symbols[0].relationships[0].is_implementation);
    }

    #[test]
    fn invalid_ranges_decode_as_none_instead_of_failing() {
        let mut doc = Vec::new();
        len(2, &occurrence(&[1, 5, 2], "s", 0), &mut doc);
        len(2, &occurrence(&[1, 2], "s", 0), &mut doc);
        let index = decode_index(&index_with(&doc), &Limits::default()).unwrap();
        assert!(index.documents[0]
            .occurrences
            .iter()
            .all(|o| o.range.is_none()));
    }

    #[test]
    fn refuses_malformed_input() {
        // A prefix ending on a field boundary is a valid message, so cut
        // inside a single top-level field.
        let mut good = Vec::new();
        len(1, b"\x1a\x09file:///p", &mut good);
        for cut in 1..good.len() {
            assert!(
                decode_index(&good[..cut], &Limits::default()).is_err(),
                "cut {cut}"
            );
        }
        // Group wire type, field number zero, overlong varint, oversized length.
        for bad in [
            &[0x0B_u8][..],
            &[0x02, 0x00][..],
            &[
                0x10, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
            ][..],
            &[0x12, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F][..],
        ] {
            assert!(decode_index(bad, &Limits::default()).is_err(), "{bad:?}");
        }
        let mut doc = Vec::new();
        len(1, &[0xFF, 0xFE], &mut doc);
        assert!(decode_index(&index_with(&doc), &Limits::default()).is_err());
    }

    #[test]
    fn enforces_limits() {
        let mut doc = Vec::new();
        len(2, &occurrence(&[0, 0, 1], "s", 0), &mut doc);
        len(2, &occurrence(&[0, 0, 1], "s", 0), &mut doc);
        let limits = Limits {
            max_occurrences: 1,
            ..Limits::default()
        };
        let e = decode_index(&index_with(&doc), &limits).unwrap_err();
        assert!(e.0.contains("occurrence limit"));
    }
}
