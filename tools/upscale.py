"""Manifest-driven HD upscale of the extracted original art.

Reads `tools/hd_assets.json` (build it with `tools/hd_manifest.py`) and writes
`game-assets/generated/<path>` at `native * scale`, mirroring the pack layout.

Two methods:

  nearest   Pure integer pixel replication, built in. Exact, seam-free and
            alpha-preserving: the safe baseline that always runs.
  external  Shell out to any upscaler with `--cmd`, then normalise the result
            back to the target size (nearest) and snap alpha to the original
            convention. Use this for Real-ESRGAN / waifu2x / xBRZ, e.g.

              --method external \\
              --cmd "realesrgan-ncnn-vulkan -i {src} -o {dst} -s {scale}"

Opaque assets (tiles) come out fully opaque; colour-keyed assets keep a hard
1-bit alpha so no soft halo survives nearest sampling. Output is
dependency-free (stdlib zlib + a small PNG codec), matching the other tools.
"""
import argparse
import json
import os
import struct
import subprocess
import sys
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "tools", "hd_assets.json")
SRC = os.path.join(ROOT, "game-assets", "extracted")
PACK = os.path.join(ROOT, "reference", "assets", "kings-bounty")
OUT = os.path.join(ROOT, "game-assets", "generated")


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


def nearest_scale(w, h, rgba, s):
    nw, nh = w * s, h * s
    out = bytearray(nw * nh * 4)
    for y in range(nh):
        sy = y // s
        src_row = sy * w * 4
        dst_row = y * nw * 4
        for x in range(nw):
            so = src_row + (x // s) * 4
            do = dst_row + x * 4
            out[do:do + 4] = rgba[so:so + 4]
    return nw, nh, bytes(out)


def nearest_resize(w, h, rgba, nw, nh):
    out = bytearray(nw * nh * 4)
    for y in range(nh):
        sy = y * h // nh
        src_row = sy * w * 4
        dst_row = y * nw * 4
        for x in range(nw):
            sx = x * w // nw
            so = src_row + sx * 4
            do = dst_row + x * 4
            out[do:do + 4] = rgba[so:so + 4]
    return nw, nh, bytes(out)


def snap_alpha(w, h, rgba, opaque, threshold):
    out = bytearray(rgba)
    for i in range(w * h):
        o = i * 4 + 3
        if opaque:
            out[o] = 255
        else:
            out[o] = 255 if out[o] >= threshold else 0
    return bytes(out)


def build_one(asset, src_path, dst_path, scale, method, cmd_tpl, opaque, threshold):
    if method == "nearest":
        w, h, rgba = read_png(src_path)
        w, h, rgba = nearest_scale(w, h, rgba, scale)
    else:
        tmp = dst_path + ".raw.png"
        command = (cmd_tpl.replace("{src}", src_path)
                   .replace("{dst}", tmp)
                   .replace("{scale}", str(scale)))
        subprocess.run(command, shell=True, check=True)
        w, h, rgba = read_png(tmp)
        os.remove(tmp)
        native = asset["native"]
        w, h, rgba = nearest_resize(w, h, rgba, native["w"] * scale, native["h"] * scale)
    rgba = snap_alpha(w, h, rgba, opaque, threshold)
    write_png(dst_path, w, h, rgba)
    return w, h


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--scale", type=int, default=4)
    ap.add_argument("--method", choices=("nearest", "external"), default="nearest")
    ap.add_argument("--cmd", default="", help="external command template using {src} {dst} {scale}")
    ap.add_argument("--alpha-threshold", type=int, default=128)
    ap.add_argument("--only", default="", help="process only paths containing this substring")
    ap.add_argument("--clean", action="store_true", help="delete the output tree first")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    if args.method == "external" and not args.cmd:
        ap.error("--method external needs --cmd")
    if args.clean and os.path.isdir(OUT):
        import shutil
        shutil.rmtree(OUT)
        print(f"cleaned {os.path.relpath(OUT, ROOT)}")

    with open(MANIFEST) as fh:
        manifest = json.load(fh)

    done = skipped = failed = 0
    for asset in manifest["assets"]:
        path = asset["path"]
        if args.only and args.only not in path:
            continue
        if asset["native"]["w"] * args.scale < 1:
            failed += 1
            continue
        dst = os.path.join(OUT, path)
        if args.dry_run:
            print(f"would upscale {path} -> {asset['native']['w'] * args.scale}"
                  f"x{asset['native']['h'] * args.scale}")
            done += 1
            continue
        src = os.path.join(PACK, path)
        if asset["source"] is not None:
            src = os.path.join(SRC, path)
        if not os.path.exists(src):
            print(f"  SKIP {path}: no source at {os.path.relpath(src, ROOT)}",
                  file=sys.stderr)
            skipped += 1
            continue
        try:
            w, h = build_one(asset, src, dst, args.scale, args.method, args.cmd,
                             asset["alpha"] == "opaque", args.alpha_threshold)
        except Exception as exc:  # noqa: BLE001
            print(f"  FAIL {path}: {exc}", file=sys.stderr)
            failed += 1
            continue
        want = (asset["native"]["w"] * args.scale, asset["native"]["h"] * args.scale)
        if (w, h) != want:
            print(f"  BAD SIZE {path}: {w}x{h}, want {want[0]}x{want[1]}", file=sys.stderr)
            failed += 1
            continue
        done += 1

    print(f"{args.scale}x {args.method}: {done} written, {skipped} skipped, {failed} failed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
