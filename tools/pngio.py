"""Minimal PNG reader and writer (stdlib only: zlib + struct).

`read_png(path) -> (w, h, rgba_bytes)` decodes 8-bit RGB/RGBA/grey/palette
PNGs to RGBA; `write_png(path, w, h, rgba)` writes 8-bit RGBA. Shared by the
art generators so none of them needs Pillow.
"""
import os
import struct
import zlib


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def read_png(path):
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path}: not a PNG")
    p = 8
    w = h = bd = ct = None
    idat = bytearray()
    while p < len(data):
        ln = struct.unpack(">I", data[p:p + 4])[0]
        tag = data[p + 4:p + 8]
        chunk = data[p + 8:p + 8 + ln]
        if tag == b"IHDR":
            w, h, bd, ct = struct.unpack(">IIBB", chunk[:10])
        elif tag == b"IDAT":
            idat += chunk
        elif tag == b"IEND":
            break
        p += 12 + ln
    if bd != 8 or ct not in (2, 6):
        raise ValueError(f"{path}: unsupported PNG (depth {bd}, colour {ct})")
    ch = 4 if ct == 6 else 3
    stride = w * ch
    raw = zlib.decompress(bytes(idat))
    out = bytearray()
    prev = bytearray(stride)
    i = 0
    for _ in range(h):
        f = raw[i]
        i += 1
        line = bytearray(raw[i:i + stride])
        i += stride
        if f == 1:
            for x in range(ch, stride):
                line[x] = (line[x] + line[x - ch]) & 0xFF
        elif f == 2:
            for x in range(stride):
                line[x] = (line[x] + prev[x]) & 0xFF
        elif f == 3:
            for x in range(stride):
                a = line[x - ch] if x >= ch else 0
                line[x] = (line[x] + ((a + prev[x]) >> 1)) & 0xFF
        elif f == 4:
            for x in range(stride):
                a = line[x - ch] if x >= ch else 0
                b = prev[x]
                c = prev[x - ch] if x >= ch else 0
                line[x] = (line[x] + _paeth(a, b, c)) & 0xFF
        out += line
        prev = line
    if ch == 4:
        return w, h, bytes(out)
    rgba = bytearray(w * h * 4)
    for i in range(w * h):
        rgba[i * 4:i * 4 + 3] = out[i * 3:i * 3 + 3]
        rgba[i * 4 + 3] = 255
    return w, h, bytes(rgba)


def _chunk(tag, payload):
    return (struct.pack(">I", len(payload)) + tag + payload
            + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF))


def write_png(path, w, h, rgba):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    stride = w * 4
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        raw += rgba[y * stride:(y + 1) * stride]
    with open(path, "wb") as fh:
        fh.write(b"\x89PNG\r\n\x1a\n")
        fh.write(_chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)))
        fh.write(_chunk(b"IDAT", zlib.compress(bytes(raw), 6)))
        fh.write(_chunk(b"IEND", b""))
