#!/usr/bin/env python3
"""Replace a SCIP index's metadata.project_root so a committed fixture
carries no machine-specific path (#1100).

Usage: normalize_index.py INPUT OUTPUT [--root file:///scip-fixture]

Only top-level field 1 (Metadata) is rewritten; its field 3 (project_root) is
replaced and every other metadata field is copied byte for byte. Documents and
external symbols are copied unchanged. Standard library only.
"""
import argparse
import sys


def read_varint(buf, pos):
    shift = value = 0
    while True:
        if pos >= len(buf) or shift > 63:
            raise ValueError("truncated or overlong varint")
        byte = buf[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, pos
        shift += 7


def write_varint(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def fields(buf):
    """Yield (field_number, wire_type, raw_field_bytes, payload)."""
    pos = 0
    while pos < len(buf):
        start = pos
        key, pos = read_varint(buf, pos)
        number, wire = key >> 3, key & 7
        if wire == 0:
            _, pos = read_varint(buf, pos)
            payload = None
        elif wire == 2:
            length, pos = read_varint(buf, pos)
            if pos + length > len(buf):
                raise ValueError("length-delimited field exceeds buffer")
            payload = buf[pos:pos + length]
            pos += length
        elif wire == 1:
            pos += 8
            payload = None
        elif wire == 5:
            pos += 4
            payload = None
        else:
            raise ValueError(f"unsupported wire type {wire}")
        if pos > len(buf):
            raise ValueError("truncated field")
        yield number, wire, buf[start:pos], payload


def length_field(number, payload):
    return write_varint(number << 3 | 2) + write_varint(len(payload)) + payload


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("input")
    parser.add_argument("output")
    parser.add_argument("--root", default="file:///scip-fixture")
    args = parser.parse_args()
    with open(args.input, "rb") as handle:
        data = handle.read()
    out = bytearray()
    for number, wire, raw, payload in fields(data):
        if number == 1 and wire == 2:
            meta = bytearray()
            for m_number, m_wire, m_raw, _ in fields(payload):
                if m_number == 3 and m_wire == 2:
                    continue
                meta += m_raw
            meta += length_field(3, args.root.encode())
            out += length_field(1, bytes(meta))
        else:
            out += raw
    with open(args.output, "xb") as handle:
        handle.write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
