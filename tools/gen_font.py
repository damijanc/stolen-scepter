"""Pack the project's own glyphs (`glyphs.py`) into the shell's font strip.

The strip is 1024x8: 128 glyphs of 8x8 indexed by byte value, white on
transparent so a draw can tint it (`shell/src/font.rs`). Control bytes and
anything `glyphs.py` lacks are left blank.

Writes the path `game.json` names in `sprites.font` and registers provenance.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from glyphs import CELL, GLYPHS, lit  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import write_png  # noqa: E402

AUTHOR = "procedural (tools/gen_font.py, glyphs from tools/glyphs.py)"
COUNT = 128


def strip():
    w = COUNT * CELL
    px = bytearray(w * CELL * 4)
    for code in range(COUNT):
        ch = chr(code)
        if ch not in GLYPHS:
            continue
        for gx, gy in lit(ch):
            if gy >= CELL:
                continue
            o = (gy * w + code * CELL + gx) * 4
            px[o:o + 4] = b"\xff\xff\xff\xff"
    return w, CELL, bytes(px)


def main():
    with open(os.path.join(PACK, "game.json")) as fh:
        rel = json.load(fh)["sprites"]["font"]
    path = os.path.join(PACK, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    write_png(path, *strip())
    register([rel], AUTHOR)
    print(f"wrote the font strip to {os.path.relpath(path, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
