"""Build the HD upscale manifest from the pack and the extracted art dump.

Reads the extraction manifest (`game-assets/extracted/manifest.json`) and the
shipped pack (`reference/assets/kings-bounty/game.json`) and writes
`tools/hd_assets.json`: every art path an HD pipeline has to cover, with its
native size, alpha convention, source frame, and whether the engine actually
draws it. Paths mirror the pack layout, so an upscaler writes
`game-assets/generated/<path>` beside the source.

Run after `tools/cc_extract.py --png-dir game-assets/extracted`:

    python3 tools/hd_manifest.py
"""
import json
import os
import struct
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXTRACTED = os.path.join(ROOT, "game-assets", "extracted")
PACK_JSON = os.path.join(ROOT, "reference", "assets", "kings-bounty", "game.json")
PACK_ART = os.path.join(ROOT, "reference", "assets", "kings-bounty")
OUT = os.path.join(ROOT, "tools", "hd_assets.json")

ROLE = {
    "classes": "class_portrait",
    "villains": "villain_portrait_or_anim",
    "troops": "troop_still_or_anim",
    "tiles": "map_tile",
    "combat": "combat_tile",
    "sprites": "hero_anim",
    "ui": "ui_or_hud",
    "font": "bitmap_font",
}

NOTE = {
    "art/sprites/hero_idle_00.png": "unused original: hero_idle candidate (cursor.256 4-7)",
    "art/tiles/tilesalt_00.png": "unused original: alternate tileset (tilesalt.256 0-8)",
    "art/ui/view_empty.png": "unused original: empty inventory placeholder (view.256 12)",
    "art/ui/end_tile_00.png": "unused original: extra ending-cartoon tile (endpic.256 2-4)",
}

DRAW_ORDER = {
    "overworld": [
        "black fill (unseen tiles and tiles with no art)",
        "per seen cell: tile.art (opaque, object art bakes its ground) OR terrain art then a transparent foe troop sprite when the foe resolves",
        "parked idle boat (transparent)",
        "hero (transparent), or the lead-troop sprite when flying",
        "fog-edge strips (3 semi-transparent black rects per unseen-facing edge; code, not art)",
        "chrome_overworld + status/bar",
    ],
    "combat": [
        "black screen",
        "combat_tile[0] grass, every cell",
        "omap code -> combat_tile (1-3 obstacles, 5-10 castle walls)",
        "units (troop anim/still; AI side mirrored)",
        "combat_tile[4] damage splat where hit_flash > 0",
        "combat_tile[11..14] target cursor while picking",
        "chrome_overworld + status",
    ],
    "end_cartoon": [
        "end_grass grid (6x5)",
        "end_carpet column (bottom-up)",
        "end_hero",
        "troop border cells (last column mirrored)",
    ],
    "location": [
        "black 240x102",
        "backdrop (240x102)",
        "animated troop at a fixed cell",
        "black below the backdrop",
    ],
    "chrome": ["hud_bar_strip at y=17", "chrome_overworld full-screen over it (transparent interior)"],
}


def png_size(path):
    with open(path, "rb") as fh:
        head = fh.read(24)
    return struct.unpack(">II", head[16:24])


def main():
    with open(os.path.join(EXTRACTED, "manifest.json")) as fh:
        dump = json.load(fh)

    # A path can be written by more than one row (grass.png is emitted from
    # comtiles, tileseta and tilesetb). The extractor writes in map order, so
    # the LAST row's render is what is on disk; keep that one.
    by_path = {}
    for rec in dump["curated"]:
        by_path[rec["path"]] = rec

    assets = []
    for path, rec in by_path.items():
        category = path.split("/")[1] if path.count("/") >= 2 else "misc"
        used = os.path.exists(os.path.join(PACK_ART, path))
        asset = {
            "path": path,
            "category": category,
            "role": ROLE.get(category, "misc"),
            "source": {
                "archive": rec["archive"],
                "entry": rec["entry"],
                "frame": rec["frame"],
            },
            "native": {"w": rec["width"], "h": rec["height"]},
            "alpha": "color_key" if rec["color_key"] else "opaque",
            "engine_used": used,
        }
        if path in NOTE:
            asset["note"] = NOTE[path]
        assets.append(asset)

    font = os.path.join(EXTRACTED, "art", "font", "kb-font.png")
    if os.path.exists(font):
        w, h = png_size(font)
        assets.append({
            "path": "art/font/kb-font.png",
            "category": "font",
            "role": "bitmap_font",
            "source": {"archive": "256", "entry": "KB.CH", "frame": None},
            "native": {"w": w, "h": h},
            "alpha": "color_key",
            "engine_used": True,
        })

    for path in ("art/ui/chrome_overworld.png", "art/ui/puzzle_cover.png"):
        full = os.path.join(PACK_ART, path)
        if not os.path.exists(full):
            continue
        w, h = png_size(full)
        assets.append({
            "path": path,
            "category": "ui",
            "role": "ui_or_hud",
            "source": None,
            "native": {"w": w, "h": h},
            "alpha": "color_key",
            "engine_used": True,
            "note": "pack-only synthetic asset with no DOS source; scaled from the pack copy, best re-authored",
        })

    assets.sort(key=lambda a: a["path"])
    used = [a for a in assets if a["engine_used"]]
    manifest = {
        "schema": 1,
        "generated_by": "tools/hd_manifest.py",
        "description": (
            "Source art for HD upscaling. Paths mirror the pack layout; write "
            "upscaled output to game-assets/generated/<path>."
        ),
        "native_cell": {"w": 48, "h": 34},
        "output_root": "game-assets/generated",
        "counts": {
            "total": len(assets),
            "engine_used": len(used),
            "unused": len(assets) - len(used),
        },
        "draw_order": DRAW_ORDER,
        "assets": assets,
    }
    with open(OUT, "w") as fh:
        json.dump(manifest, fh, indent=1)
        fh.write("\n")
    print(f"{OUT}: {len(assets)} assets ({len(used)} engine-used, "
          f"{len(assets) - len(used)} unused originals)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
