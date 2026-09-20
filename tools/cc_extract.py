#!/usr/bin/env python3
"""
Extract the original King's Bounty DOS `.CC` archives (256.CC / 416.CC).

Format (matches OpenBounty's `reference/tools/extract_lzw.c`, and verified
byte-exact against both shipped archives):

    u16 LE   entry count
    count x directory rows of 8 bytes:
        u16 LE   filename hash (the original name is NOT stored)
        u24 LE   absolute offset of the compressed blob
        u24 LE   compressed blob size

    each blob: u32 LE uncompressed size, then an LZW stream
    LZW: 9..12 bit codes, LSB-first within each byte,
         0x100 = dictionary reset, 0x101 = end of stream

Entries are contiguous and the last one ends exactly at EOF, which makes the
directory trivial to validate.

Findings from the shipped DOS release, so nobody repeats the search:
  * 256.CC holds 66 entries, 416.CC holds 133. Every entry decodes to exactly
    its declared uncompressed size.
  * Both archives are PURE GRAPHICS. There is no English text in either --
    what looks like strings is pixel-run data. The game's text (including the
    manual-lookup copy-protection prompt) lives in KB.EXE, which is packed:
    no plaintext, no single-byte XOR, no bit-7-terminator encoding, and the
    longest printable run in the whole 79KB binary is 14 bytes of noise.

Usage:
    ./cc_extract.py 256.CC                 # list entries
    ./cc_extract.py 256.CC --out DIR       # also write decoded blobs
"""
import argparse
import os
import re
import struct
import sys

LZW_RESET, LZW_END = 0x100, 0x101


def lzw_decode(payload: bytes, expected: int) -> bytes:
    """Decode one LZW blob. `expected` is the declared uncompressed size."""
    out = bytearray()
    parent = [-1] * 4096
    value = [0] * 4096
    nxt, width, prev = 0x102, 9, -1
    bit, total_bits = 0, len(payload) * 8

    def read():
        nonlocal bit
        if bit + width > total_bits:
            return -1
        v = 0
        for i in range(width):
            v |= ((payload[(bit + i) >> 3] >> ((bit + i) & 7)) & 1) << i
        bit += width
        return v

    def expand(code):
        buf = bytearray()
        while code >= 0x102:
            buf.append(value[code])
            code = parent[code]
        buf.append(code if code < 0x100 else 0)
        buf.reverse()
        return buf

    while True:
        code = read()
        if code < 0 or code == LZW_END:
            break
        if code == LZW_RESET:
            nxt, width, prev = 0x102, 9, -1
            continue
        if prev == -1:
            seq = expand(code)
        else:
            seq = expand(code) if code < nxt else (lambda s: s + s[:1])(expand(prev))
            if nxt < 4096:
                parent[nxt], value[nxt] = prev, seq[0]
                nxt += 1
                if nxt >= (1 << width) and width < 12:
                    width += 1
        out += seq
        prev = code
        if expected and len(out) >= expected:
            break
    return bytes(out[:expected]) if expected else bytes(out)


def directory(data: bytes):
    """Yield (index, name_hash, uncompressed_size, payload) per entry."""
    count = struct.unpack_from('<H', data, 0)[0]
    off = 2
    for i in range(count):
        tag = struct.unpack_from('<H', data, off)[0]
        blob_off = int.from_bytes(data[off + 2:off + 5], 'little')
        blob_len = int.from_bytes(data[off + 5:off + 8], 'little')
        off += 8
        blob = data[blob_off:blob_off + blob_len]
        if len(blob) < 4:
            continue
        usize = struct.unpack_from('<I', blob, 0)[0]
        yield i, tag, usize, blob[4:]


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('archive')
    ap.add_argument('--out', help='directory to write decoded entries into')
    args = ap.parse_args()

    data = open(args.archive, 'rb').read()
    count = struct.unpack_from('<H', data, 0)[0]
    print(f"{args.archive}: {count} entries, {len(data)} bytes")

    if args.out:
        os.makedirs(args.out, exist_ok=True)

    bad = 0
    for i, tag, usize, payload in directory(data):
        try:
            raw = lzw_decode(payload, usize)
        except Exception as exc:                      # noqa: BLE001
            print(f"  entry {i:3} tag={tag:#06x} DECODE FAILED: {exc}")
            bad += 1
            continue
        ok = "ok" if len(raw) == usize else f"SHORT {len(raw)}"
        words = len(re.findall(rb'[A-Za-z]{4,}', raw))
        print(f"  entry {i:3} tag={tag:#06x} size={usize:7} {ok:>10}  alpha-runs={words}")
        if args.out:
            path = os.path.join(args.out, f"e{i:03d}_{tag:04x}.bin")
            with open(path, 'wb') as fh:
                fh.write(raw)
    if bad:
        print(f"{bad} entries failed", file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
