#!/usr/bin/env python3
"""
Extract the original King's Bounty DOS `.CC` archives (256.CC / 416.CC).

Two jobs:

  1. List / validate / dump the raw LZW entries (the original behaviour).
  2. Decode the VGA `.256` graphics atlases into PNGs, plus the bitmap font,
     a palette swatch and an index gallery, so the original art can be used
     as a source for a visual overhaul.

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

Filenames are recovered by hashing candidate names (graphics bases x
suffixes + the specials) and matching the 16-bit hash, porting cc_hash()
from `reference/tools/extract_lzw.c`.

The VGA files are multi-image atlases (port of `extract_vga.c`):

    u16 LE   image count
    per-image directory (count x 4 bytes):
        u16 LE   pixel-data offset
        u16 LE   mask offset (0 = auto-derive)
    per-image at pixel-data offset:
        u16 LE   width
        u16 LE   height
        w*h bytes of 8bpp palette indices
        (color-key byte at mask_off or right after the pixels)

The palette lives inside MCGA.DRV at 0x032D (768 bytes, 6-bit RGB); PNG
output expands 6->8 bits by bit replication, matching the shipped art.

Findings from the shipped DOS release, so nobody repeats the search:
  * 256.CC holds 66 entries, 416.CC holds 133. Every entry decodes to exactly
    its declared uncompressed size.
  * Both archives are PURE GRAPHICS. There is no English text in either --
    what looks like strings is pixel-run data. The game's text (including the
    manual-lookup copy-protection prompt) lives in KB.EXE, which is packed:
    no plaintext, no single-byte XOR, no bit-7-terminator encoding, and the
    longest printable run in the whole 79KB binary is 14 bytes of noise.
  * 416.CC is mostly CGA/EGA siblings (`.4` / `.16`). Only its `endpic.256`
    (win/lose ending pictures) is VGA, and this tool decodes VGA only.

Usage:
    ./cc_extract.py 256.CC                       # list entries
    ./cc_extract.py 256.CC --out DIR             # also write decoded blobs
    ./cc_extract.py --png-dir DIR 256.CC 416.CC  # decode art to PNGs
    ./cc_extract.py --png-dir DIR --from-zip game-assets/Kings-Bounty_DOS_EN.zip

No pip dependencies -- the PNG writer is a small stdlib `zlib` encoder.
"""
import argparse
import os
import re
import struct
import sys
import tempfile
import zipfile
import zlib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cc_output_map import OUTPUT_MAP  # noqa: E402  (curated frame -> art path)

LZW_RESET, LZW_END = 0x100, 0x101
PALETTE_OFF = 0x032D


# ---- LZW + CC directory ------------------------------------------------

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


# ---- filename hash recovery (port of cc_hash in extract_lzw.c) ---------

def cc_hash(name: str) -> int:
    key = 0
    for ch in name:
        n = ord(ch) & 0x7F
        if n >= 0x60:
            n -= 0x20
        key = ((key >> 8) | (key << 8)) & 0xFFFF
        key = ((key << 1) | (key >> 15)) & 0xFFFF
        key = (key + n) & 0xFFFF
    return key


GRAPHIC_BASES = [
    "peas", "spri", "mili", "wolf", "skel", "zomb", "gnom", "orcs", "arcr", "elfs",
    "pike", "noma", "dwar", "ghos", "kght", "ogre", "brbn", "trol", "cavl", "drui",
    "arcm", "vamp", "gian", "demo", "drag",
    "mury", "hack", "ammi", "baro", "drea", "cane", "mora", "barr", "barg", "rina",
    "ragf", "mahk", "auri", "czar", "magu", "urth", "arec",
    "knig", "pala", "sorc", "barb",
    "nwcp", "title", "select",
    "tileseta", "tilesetb", "tilesalt",
    "cursor", "town", "cstl", "plai", "frst", "dngn", "cave",
    "comtiles", "view", "endpic",
]
GRAPHIC_SUFFIXES = [".4", ".16", ".256"]
OTHER_FILES = [
    "KB.CH", "LAND.ORG",
    "TIMER.DRV", "SOUND.DRV",
    "CGA.DRV", "EGA.DRV", "TGA.DRV", "HGA.DRV", "MCGA.DRV",
]


def build_name_table():
    table = {}
    for base in GRAPHIC_BASES:
        for suffix in GRAPHIC_SUFFIXES:
            name = base + suffix
            table[cc_hash(name)] = name
    for name in OTHER_FILES:
        table[cc_hash(name)] = name
    return table


def is_tile_entry(name: str) -> bool:
    return name.startswith("tileset") or name.startswith("tilesalt")


# ---- archives ----------------------------------------------------------

class Archive:
    """One decoded .CC archive with names resolved and entries cached."""

    def __init__(self, path):
        self.path = path
        self.stem = os.path.splitext(os.path.basename(path))[0]
        data = open(path, 'rb').read()
        table = build_name_table()
        self.entries = []          # list of (name, decoded bytes)
        self._by_name = {}
        for _, tag, usize, payload in directory(data):
            name = table.get(tag, f"unk_{tag:04x}")
            try:
                raw = lzw_decode(payload, usize)
            except Exception as exc:                       # noqa: BLE001
                print(f"  {name}: DECODE FAILED: {exc}", file=sys.stderr)
                continue
            if len(raw) != usize:
                print(f"  {name}: SHORT {len(raw)}/{usize}", file=sys.stderr)
            self.entries.append((name, raw))
            self._by_name.setdefault(name, raw)

    def find(self, name):
        return self._by_name.get(name)


class Loader:
    """A set of archives searchable by entry name, 256.CC first if present."""

    def __init__(self, paths):
        archives = [Archive(p) for p in paths]
        # Prefer 256.CC (the VGA archive) when an entry name appears twice.
        archives.sort(key=lambda a: 0 if '256' in a.stem else 1)
        self.archives = archives

    def find(self, name):
        for arc in self.archives:
            raw = arc.find(name)
            if raw is not None:
                return arc, raw
        return None, None

    def palette(self):
        _, raw = self.find("MCGA.DRV")
        if raw is None or len(raw) < PALETTE_OFF + 768:
            return None
        return raw[PALETTE_OFF:PALETTE_OFF + 768]

    def font(self):
        _, raw = self.find("KB.CH")
        return raw


# ---- VGA atlas decoder (port of extract_vga.c) -------------------------

def _u16(data, off):
    return data[off] | (data[off + 1] << 8)


def frame_count(data):
    if len(data) < 2:
        return 0
    count = _u16(data, 0)
    return count if 0 < count <= 1024 else 0


def detect_real_bg(w, h, pixels, claimed_key):
    """Border dominance heuristic, porting detect_real_bg(). If >=50% of the
    1-pixel border is one color and it differs from the claimed key, the key
    is wrong; return the border color instead. Tie-break: first-seen wins."""
    counts = [0] * 256
    first_seen = [1 << 30] * 256
    border_size = 0
    seq = 0

    def tally(pix):
        nonlocal seq, border_size
        if counts[pix] == 0:
            first_seen[pix] = seq
        counts[pix] += 1
        seq += 1
        border_size += 1

    for x in range(w):
        tally(pixels[x])
    for x in range(w):
        tally(pixels[(h - 1) * w + x])
    for y in range(1, h - 1):
        tally(pixels[y * w])
        tally(pixels[y * w + (w - 1)])
    if border_size == 0:
        return claimed_key

    best_idx, best_count, best_first = -1, 0, 1 << 30
    for i in range(256):
        if counts[i] > best_count or (counts[i] == best_count and first_seen[i] < best_first):
            best_idx, best_count, best_first = i, counts[i], first_seen[i]
    if best_count * 100 // border_size >= 50 and best_idx != claimed_key:
        return best_idx
    return claimed_key


def render_frame(data, frame_idx, palette, apply_color_key):
    """Render one frame to (w, h, RGBA bytes), or None on a malformed frame."""
    count = frame_count(data)
    if not 0 <= frame_idx < count:
        return None
    dir_off = 2 + frame_idx * 4
    if dir_off + 4 > len(data):
        return None
    off = _u16(data, dir_off)
    mask_off = _u16(data, dir_off + 2)
    if off + 4 > len(data):
        return None
    w, h = _u16(data, off), _u16(data, off + 2)
    if not (0 < w <= 1024 and 0 < h <= 1024):
        return None
    need = w * h
    if off + 4 + need > len(data):
        return None
    pixels = data[off + 4:off + 4 + need]

    key = -1
    if apply_color_key:
        color_key = -1
        if mask_off != 0 and mask_off < len(data):
            color_key = data[mask_off]
        else:
            auto_off = off + 4 + need
            if auto_off < len(data):
                color_key = data[auto_off]
        if color_key >= 0:
            key = detect_real_bg(w, h, pixels, color_key)

    rgba = bytearray(need * 4)
    for i, pidx in enumerate(pixels):
        if key >= 0 and pidx == key:
            continue
        o = i * 4
        r6 = palette[pidx * 3] & 0x3F
        g6 = palette[pidx * 3 + 1] & 0x3F
        b6 = palette[pidx * 3 + 2] & 0x3F
        rgba[o] = (r6 << 2) | (r6 >> 4)
        rgba[o + 1] = (g6 << 2) | (g6 >> 4)
        rgba[o + 2] = (b6 << 2) | (b6 >> 4)
        rgba[o + 3] = 0xFF
    return w, h, bytes(rgba)


# ---- minimal PNG writer (RGBA8, stdlib only) ---------------------------

def _png_chunk(tag, data):
    return (struct.pack('>I', len(data)) + tag + data
            + struct.pack('>I', zlib.crc32(tag + data) & 0xFFFFFFFF))


def write_png(path, w, h, rgba):
    parent = os.path.dirname(path)
    if parent:
        os.makedirs(parent, exist_ok=True)
    stride = w * 4
    raw = bytearray()
    for y in range(h):
        raw.append(0)                      # filter: None
        raw += rgba[y * stride:(y + 1) * stride]
    ihdr = struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)
    with open(path, 'wb') as fh:
        fh.write(b'\x89PNG\r\n\x1a\n')
        fh.write(_png_chunk(b'IHDR', ihdr))
        fh.write(_png_chunk(b'IDAT', zlib.compress(bytes(raw), 6)))
        fh.write(_png_chunk(b'IEND', b''))


# ---- font + palette helpers -------------------------------------------

def font_glyph(font, ch):
    """Set (x, y) pixels of an 8x8 MSB-first glyph from KB.CH."""
    if font is None:
        return []
    code = ord(ch) & 0x7F
    base = code * 8
    if base + 8 > len(font):
        return []
    pts = []
    for row in range(8):
        bits = font[base + row]
        for col in range(8):
            if bits & (0x80 >> col):
                pts.append((col, row))
    return pts


def emit_font(font, out_dir):
    if font is None or len(font) != 1024:
        print("  KB.CH missing or unexpected size; skipping font", file=sys.stderr)
        return
    width, height = 128 * 8, 8
    rgba = bytearray(width * height * 4)
    for g in range(128):
        for row in range(8):
            bits = font[g * 8 + row]
            for col in range(8):
                if bits & (0x80 >> col):
                    o = (row * width + g * 8 + col) * 4
                    rgba[o:o + 4] = b'\xff\xff\xff\xff'
    write_png(os.path.join(out_dir, "art", "font", "kb-font.png"),
              width, height, bytes(rgba))


def render_palette(palette, font):
    """16x16 swatch grid, 32x32 per cell: 24px color + two-digit index."""
    cell, n = 32, 16
    width = height = cell * n
    rgba = bytearray(width * height * 4)
    for idx in range(256):
        cx, cy = (idx % n) * cell, (idx // n) * cell
        r6 = palette[idx * 3] & 0x3F
        g6 = palette[idx * 3 + 1] & 0x3F
        b6 = palette[idx * 3 + 2] & 0x3F
        r, g, b = (r6 << 2) | (r6 >> 4), (g6 << 2) | (g6 >> 4), (b6 << 2) | (b6 >> 4)
        # Dark cell background so unlabeled space is not transparent.
        for y in range(cell):
            for x in range(cell):
                o = ((cy + y) * width + (cx + x)) * 4
                rgba[o:o + 4] = b'\x00\x00\x00\xff'
        for y in range(24):
            for x in range(cell):
                o = ((cy + y) * width + (cx + x)) * 4
                rgba[o], rgba[o + 1], rgba[o + 2] = r, g, b
        lum = (r * 299 + g * 587 + b * 114) // 1000
        fg = (0, 0, 0) if lum > 140 else (255, 255, 255)
        for di, ch in enumerate(f"{idx:02d}"):
            for (px, py) in font_glyph(font, ch):
                o = ((cy + 24 + py) * width + (cx + 8 + di * 8 + px)) * 4
                rgba[o], rgba[o + 1], rgba[o + 2], rgba[o + 3] = fg[0], fg[1], fg[2], 255
    return width, height, bytes(rgba)


def palette_hex(palette):
    return ["#%02x%02x%02x" % (
        ((palette[i * 3] & 0x3F) << 2) | ((palette[i * 3] & 0x3F) >> 4),
        ((palette[i * 3 + 1] & 0x3F) << 2) | ((palette[i * 3 + 1] & 0x3F) >> 4),
        ((palette[i * 3 + 2] & 0x3F) << 2) | ((palette[i * 3 + 2] & 0x3F) >> 4),
    ) for i in range(256)]


# ---- PNG extraction ----------------------------------------------------

def extract_art(loader, out_dir):
    palette = loader.palette()
    if palette is None:
        print("MCGA.DRV missing; cannot extract art", file=sys.stderr)
        return 1
    font = loader.font()
    os.makedirs(out_dir, exist_ok=True)

    raw_records = []
    for arc in loader.archives:
        for name, data in arc.entries:
            if not name.endswith(".256"):
                continue
            count = frame_count(data)
            for frame in range(count):
                rendered = render_frame(data, frame, palette, not is_tile_entry(name))
                if rendered is None:
                    print(f"  render fail: {arc.stem}/{name} frame {frame}",
                          file=sys.stderr)
                    continue
                w, h, rgba = rendered
                rel = f"raw/{arc.stem}/{name}/frame_{frame:02d}.png"
                write_png(os.path.join(out_dir, rel), w, h, rgba)
                raw_records.append({
                    "path": rel, "archive": arc.stem, "entry": name,
                    "frame": frame, "width": w, "height": h,
                    "color_key": not is_tile_entry(name),
                })

    curated_records = []
    for raw_name, frame, out_rel in OUTPUT_MAP:
        entry_name = raw_name[4:] if raw_name.startswith("416:") else raw_name
        arc, data = loader.find(entry_name)
        if data is None:
            print(f"  missing entry {entry_name} for {out_rel}", file=sys.stderr)
            continue
        color_key = not is_tile_entry(entry_name)
        rendered = render_frame(data, frame, palette, color_key)
        if rendered is None:
            print(f"  render fail: {entry_name} frame {frame}", file=sys.stderr)
            continue
        w, h, rgba = rendered
        write_png(os.path.join(out_dir, out_rel), w, h, rgba)
        curated_records.append({
            "path": out_rel, "archive": arc.stem, "entry": entry_name,
            "frame": frame, "width": w, "height": h, "color_key": color_key,
        })

    emit_font(font, out_dir)
    write_png(os.path.join(out_dir, "palette.png"),
              *render_palette(palette, font))
    write_manifest(out_dir, palette, curated_records, raw_records)
    write_index(out_dir, palette, curated_records, raw_records)

    print(f"art -> {out_dir}")
    print(f"  {len(curated_records)} curated PNGs, {len(raw_records)} raw frames")
    return 0


def write_manifest(out_dir, palette, curated, raw):
    import json
    manifest = {
        "generated_by": "tools/cc_extract.py",
        "palette": palette_hex(palette),
        "curated": curated,
        "raw": raw,
    }
    with open(os.path.join(out_dir, "manifest.json"), "w") as fh:
        json.dump(manifest, fh, indent=1)


def _figure(record):
    return (f'<figure><img src="{record["path"]}" loading="lazy" '
            f'alt="{record["path"]}"><figcaption>{os.path.basename(record["path"])}'
            f'<br><span>{record["width"]}x{record["height"]} '
            f'&middot; {record["entry"]}#{record["frame"]}</span></figcaption></figure>')


def write_index(out_dir, palette, curated, raw):
    by_category = {}
    for rec in curated:
        parts = rec["path"].split("/")
        category = parts[1] if len(parts) > 2 else "misc"
        by_category.setdefault(category, []).append(rec)

    raw_by_entry = {}
    for rec in raw:
        raw_by_entry.setdefault(f'{rec["archive"]}/{rec["entry"]}', []).append(rec)

    swatches = "".join(
        f'<span style="background:{hexv}" title="{i}"></span>'
        for i, hexv in enumerate(palette_hex(palette)))

    sections = []
    for category in sorted(by_category):
        recs = sorted(by_category[category], key=lambda r: r["path"])
        sections.append(
            f'<section><h2>{category} <small>({len(recs)})</small></h2>'
            f'<div class="grid">{"".join(_figure(r) for r in recs)}</div></section>')

    raw_sections = []
    for entry in sorted(raw_by_entry):
        recs = sorted(raw_by_entry[entry], key=lambda r: r["frame"])
        raw_sections.append(
            f'<details><summary>{entry} <small>({len(recs)})</small></summary>'
            f'<div class="grid">{"".join(_figure(r) for r in recs)}</div></details>')

    html = f"""<!doctype html>
<meta charset="utf-8">
<title>King's Bounty - extracted original art</title>
<style>
 body {{ font-family: system-ui, sans-serif; margin: 1.5rem; background:#1b1b1f; color:#e8e8ea; }}
 h1, h2 {{ font-weight: 600; }}
 small {{ color:#9a9aa2; font-weight: 400; }}
 .palette {{ display:grid; grid-template-columns:repeat(32,1fr); width:min(100%,640px);
             border:1px solid #3a3a42; margin:.5rem 0 1.5rem; }}
 .palette span {{ aspect-ratio:1; }}
 .grid {{ display:grid; grid-template-columns:repeat(auto-fill,minmax(96px,1fr)); gap:.75rem; }}
 figure {{ margin:0; background:#26262c; border:1px solid #34343c; border-radius:6px;
           padding:.5rem; text-align:center; }}
 figure img {{ max-width:100%; image-rendering: pixelated;
               background:
                 linear-gradient(45deg,#3a3a42 25%,transparent 25%,transparent 75%,#3a3a42 75%),
                 linear-gradient(45deg,#3a3a42 25%,#2e2e34 25%,#2e2e34 75%,#3a3a42 75%);
               background-size:16px 16px; background-position:0 0,8px 8px; }}
 figcaption {{ font-size:.7rem; margin-top:.35rem; word-break:break-all; }}
 figcaption span {{ color:#8f8f98; }}
 section {{ margin-bottom:2rem; }}
 details {{ margin:.5rem 0; }}
 summary {{ cursor:pointer; padding:.35rem 0; }}
</style>
<h1>Extracted original art</h1>
<p>Source: King's Bounty DOS <code>256.CC</code> + <code>416.CC</code>, decoded by
<code>tools/cc_extract.py</code>. {len(curated)} curated engine-role PNGs and
{len(raw)} raw VGA frames. <code>chrome_overworld.png</code> and
<code>puzzle_cover.png</code> are port-only synth assets and are not emitted.</p>
<h2>VGA palette</h2>
<div class="palette">{swatches}</div>
{''.join(sections)}
<h2>All source frames</h2>
{''.join(raw_sections)}
"""
    with open(os.path.join(out_dir, "index.html"), "w") as fh:
        fh.write(html)


# ---- listing / raw dump (original behaviour) ---------------------------

def report_archive(path, out_dir, name_table):
    data = open(path, 'rb').read()
    count = struct.unpack_from('<H', data, 0)[0]
    print(f"{path}: {count} entries, {len(data)} bytes")
    bad = 0
    for i, tag, usize, payload in directory(data):
        name = name_table.get(tag, f"unk_{tag:04x}")
        try:
            raw = lzw_decode(payload, usize)
        except Exception as exc:                           # noqa: BLE001
            print(f"  entry {i:3} {name:<12} tag={tag:#06x} DECODE FAILED: {exc}")
            bad += 1
            continue
        ok = "ok" if len(raw) == usize else f"SHORT {len(raw)}"
        print(f"  entry {i:3} {name:<12} tag={tag:#06x} size={usize:7} {ok:>10}")
        if out_dir:
            stem = os.path.splitext(os.path.basename(path))[0]
            dest = os.path.join(out_dir, f"{stem}_e{i:03d}_{tag:04x}_{name}")
            with open(dest, 'wb') as fh:
                fh.write(raw)
    if bad:
        print(f"{bad} entries failed", file=sys.stderr)
        return 1
    return 0


def main():
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('archive', nargs='*', help='one or more .CC archives')
    ap.add_argument('--out', help='directory to write decoded raw blobs into')
    ap.add_argument('--png-dir',
                    help='directory to extract decoded art PNGs into')
    ap.add_argument('--from-zip',
                    help='unzip a DOS distribution zip and use its .CC files')
    args = ap.parse_args()

    paths = list(args.archive)
    if args.from_zip:
        scratch = tempfile.mkdtemp(prefix='kb-cc-')
        with zipfile.ZipFile(args.from_zip) as zf:
            zf.extractall(scratch)
        for root, _, files in os.walk(scratch):
            for fn in files:
                if fn.upper().endswith('.CC'):
                    paths.append(os.path.join(root, fn))
        paths.sort()
    if not paths:
        ap.error('provide at least one .CC archive (or --from-zip)')

    if args.out:
        os.makedirs(args.out, exist_ok=True)

    name_table = build_name_table()
    rc = 0
    for path in paths:
        rc |= report_archive(path, args.out, name_table)

    if args.png_dir:
        rc |= extract_art(Loader(paths), args.png_dir)

    return rc


if __name__ == '__main__':
    sys.exit(main())
