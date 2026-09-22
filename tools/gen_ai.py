"""Drive a local ComfyUI server to generate the open art pack from the spec.

Two steps, meant to be run in order:

    python3 tools/gen_ai.py probe
        Reads the running ComfyUI server: lists checkpoints and finds a
        background-removal node. Prints what the generator will use.

    python3 tools/gen_ai.py one art/tiles/grass.png
        Generates one slot: SDXL -> (optional background removal) -> downscale
        to native*scale -> colour grade -> writes to game-assets/ai-out/ and
        records provenance. Nothing is written to the committed pack.

Stdlib only. ComfyUI must be running (default http://127.0.0.1:8188).
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from upscale import read_png, write_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "tools", "hd_assets.json")
PROVENANCE = os.path.join(ROOT, "packs", "open", "provenance.json")
OUT = os.path.join(ROOT, "game-assets", "ai-out")

CKPT = "sd_xl_base_1.0.safetensors"
GEN_W, GEN_H = 896, 640
NEG = ("text, watermark, ui, border, frame, perspective, horizon, people, buildings, "
       "multiple characters, group, crowd, collage, grid, split image, duplicates, "
       "signature, blurry, low quality, deformed")

TROOP = {
    "peasants": "peasant villager holding a pitchfork",
    "militia": "militia soldier with a sword and shield",
    "pikemen": "pikeman soldier holding a long pike",
    "archers": "archer drawing a bow",
    "knights": "armoured knight with a sword and shield",
    "barbarians": "barbarian warrior with a battle axe",
    "nomads": "nomad warrior with a scimitar",
    "orcs": "orc warrior with an axe",
    "ogres": "ogre warrior with a club",
    "giants": "giant with a club",
    "trolls": "troll with a club",
    "zombies": "shambling zombie",
    "skeletons": "skeleton warrior with a sword",
    "vampires": "vampire lord with a dark cape",
    "elves": "elf archer",
    "dwarves": "dwarf warrior with an axe",
    "gnomes": "gnome with a pointed hat",
    "druids": "druid with a staff",
    "archmages": "archmage wizard with a staff",
    "demons": "winged demon warrior",
    "wolves": "dire wolf",
    "cavalry": "mounted knight on a horse",
    "dragons": "dragon",
    "ghosts": "ghostly spirit",
    "sprites": "tiny fairy sprite",
}

TERRAIN = {
    "grass": "lush green grass meadow",
    "grass_variant": "green grass with small tufts and tiny wildflowers",
    "water": "deep blue ocean water with gentle ripples",
    "forest": "dense green forest canopy from above",
    "mountain": "grey rocky mountain terrain and stone ridges",
    "desert": "golden sand dunes",
}


def http_get(host, path, raw=False):
    with urllib.request.urlopen(host + path, timeout=30) as r:
        data = r.read()
    return data if raw else json.loads(data)


def http_post(host, path, obj):
    body = json.dumps(obj).encode()
    req = urllib.request.Request(host + path, data=body,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")
        raise RuntimeError(f"HTTP {e.code} from {path}: {detail}") from None


def find_matte(info):
    for cls, spec in info.items():
        blob = (cls + " " + str(spec.get("display_name", ""))).lower()
        if "rmbg" in blob or ("remove" in blob and "background" in blob):
            return cls
    return None


def cmd_probe(args):
    info = http_get(args.host, "/object_info")
    ckpts = info.get("CheckpointLoaderSimple", {}).get("input", {}).get("required", {}) \
                .get("ckpt_name", [[]])[0]
    print("checkpoints:", ckpts)
    matte = find_matte(info)
    print("matte node:", matte)
    if matte:
        req = info[matte]["input"].get("required", {})
        print("matte inputs:", {k: v[0] if isinstance(v, list) else v for k, v in req.items()})
        print("matte outputs:", info[matte].get("output_name", info[matte].get("output")))
    print("prompt endpoint reachable:", http_get(args.host, "/queue") is not None)
    return 0


def prompt_for(path):
    name = os.path.basename(path)
    stem = name[:-4]
    if path.startswith("art/tiles/") and stem in TERRAIN:
        return (f"seamless top-down terrain texture, {TERRAIN[stem]}, painterly digital "
                f"painting, fantasy strategy game art, soft even lighting, high detail"), False
    if path.startswith("art/troops/"):
        unit = stem.rsplit("_", 1)[0]
        desc = TROOP.get(unit, unit)
        return (f"a single {desc}, full body, side view, centered, one character only, "
                f"painterly fantasy strategy game unit sprite, plain flat neutral "
                f"background, soft light, high detail"), True
    return (f"{stem}, painterly fantasy strategy game art, high detail, centered"), True


def matte_image_slot(info, cls):
    spec = info.get(cls, {})
    names = spec.get("output_name", [])
    types = spec.get("output", [])
    for i, n in enumerate(names):
        if str(n).upper() == "IMAGE":
            return i
    for i, t in enumerate(types):
        if str(t).upper() == "IMAGE":
            return i
    return 0


def matte_node_inputs(info, cls, image_ref):
    spec = info.get(cls, {}).get("input", {}).get("required", {})
    inputs = {"image": image_ref}
    for name, t in spec.items():
        if name == "image":
            continue
        typ = t[0]
        opts = t[1] if len(t) > 1 and isinstance(t[1], dict) else {}
        if isinstance(typ, list):
            inputs[name] = opts.get("default", typ[0])
        elif "default" in opts:
            inputs[name] = opts["default"]
    return inputs


def build_graph(ckpt, positive, negative, seed, w, h, matte):
    g = {
        "4": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": ckpt}},
        "5": {"class_type": "EmptyLatentImage",
              "inputs": {"width": w, "height": h, "batch_size": 1}},
        "6": {"class_type": "CLIPTextEncode",
              "inputs": {"text": positive, "clip": ["4", 1]}},
        "7": {"class_type": "CLIPTextEncode",
              "inputs": {"text": negative, "clip": ["4", 1]}},
        "3": {"class_type": "KSampler",
              "inputs": {"seed": seed, "steps": 30, "cfg": 6.0,
                         "sampler_name": "dpmpp_2m", "scheduler": "karras",
                         "denoise": 1.0, "model": ["4", 0],
                         "positive": ["6", 0], "negative": ["7", 0],
                         "latent_image": ["5", 0]}},
        "8": {"class_type": "VAEDecode", "inputs": {"samples": ["3", 0], "vae": ["4", 2]}},
    }
    src = ["8", 0]
    if matte:
        cls, inputs, slot = matte
        g["9"] = {"class_type": cls, "inputs": inputs}
        src = ["9", slot]
    g["10"] = {"class_type": "SaveImage", "inputs": {"images": src, "filename_prefix": "ob"}}
    return g


def generate(host, graph, timeout=600):
    r = http_post(host, "/prompt", {"prompt": graph})
    if r.get("node_errors"):
        raise RuntimeError(f"node errors: {r['node_errors']}")
    pid = r["prompt_id"]
    start = time.time()
    while time.time() - start < timeout:
        hist = http_get(host, f"/history/{pid}")
        if pid in hist:
            outputs = hist[pid].get("outputs", {})
            for node in outputs.values():
                for img in node.get("images", []):
                    if img.get("type") in ("output", None):
                        q = (f"/view?filename={urllib.parse.quote(img['filename'])}"
                             f"&subfolder={urllib.parse.quote(img.get('subfolder',''))}"
                             f"&type=output")
                        return http_get(host, q, raw=True)
        time.sleep(2)
    raise TimeoutError("generation timed out")


def box_downscale(w, h, rgba, nw, nh):
    out = bytearray(nw * nh * 4)
    for y in range(nh):
        y0, y1 = y * h // nh, max(y * h // nh + 1, (y + 1) * h // nh)
        for x in range(nw):
            x0, x1 = x * w // nw, max(x * w // nw + 1, (x + 1) * w // nw)
            acc = [0, 0, 0, 0]
            n = 0
            for yy in range(y0, y1):
                row = yy * w * 4
                for xx in range(x0, x1):
                    o = row + xx * 4
                    for c in range(4):
                        acc[c] += rgba[o + c]
                    n += 1
            o = (y * nw + x) * 4
            for c in range(4):
                out[o + c] = acc[c] // n
    return nw, nh, bytes(out)


def grade(w, h, rgba, saturation=1.08, contrast=1.06, lift=4):
    out = bytearray(rgba)
    for i in range(w * h):
        o = i * 4
        r, g, b = out[o], out[o + 1], out[o + 2]
        lum = (r * 299 + g * 587 + b * 114) // 1000
        for c, v in enumerate((r, g, b)):
            v = lum + (v - lum) * saturation
            v = (v - 128) * contrast + 128 + lift
            out[o + c] = max(0, min(255, int(v)))
    return bytes(out)


SIDES = {"01": ("outer", ("N", "W")), "02": ("outer", ("S", "W")),
         "03": ("outer", ("N", "E")), "04": ("outer", ("S", "E")),
         "05": ("inner", "SE"), "06": ("inner", "NE"),
         "07": ("inner", "SW"), "08": ("inner", "NW"),
         "09": ("side", "E"), "10": ("side", "W"), "11": ("side", "N"), "12": ("side", "S")}
WATER_SIDES = {"00": ("outer", ("N", "E")), "01": ("outer", ("N", "W")),
               "02": ("outer", ("S", "W")), "03": ("outer", ("S", "E")),
               "04": ("inner", "SE"), "05": ("inner", "NE"),
               "06": ("inner", "SW"), "07": ("inner", "NW"),
               "08": ("side", "E"), "09": ("side", "W"), "10": ("side", "N"), "11": ("side", "S")}

VILLAIN = {}

OBJECT = {
    "town": "a small medieval fantasy town, cluster of houses with red roofs",
    "chest": "a closed wooden treasure chest with iron bands",
    "sign": "a wooden signpost with a blank board",
    "bridge_h": "a wooden bridge crossing water, seen from above, horizontal",
    "bridge_v": "a wooden bridge crossing water, seen from above, vertical",
    "dwelling_plains": "a round thatched hut",
    "dwelling_forest": "a wooden hut with a leafy roof",
    "dwelling_hills": "a cave entrance in a rocky hill",
    "dwelling_dungeon": "a dark stone doorway with pillars",
    "artifact_chest": "an ornate chest with a glowing gem",
    "artifact_ring": "a golden ring with a gem",
    "wandering_army": "a dark banner on a pole beside an armoured figure",
}

COMBAT = {
    "obstacle_01": "a grey boulder rock",
    "obstacle_02": "a single green tree",
    "obstacle_03": "a grassy earth mound",
    "castle_spike": "a red and orange impact burst splash",
    "cursor_01": "a golden target ring", "cursor_02": "a golden target ring",
    "cursor_03": "a golden target ring", "cursor_04": "a golden target ring",
    "castle_wall_01": "a stone castle wall segment", "castle_wall_02": "a stone castle wall segment",
    "castle_wall_03": "a stone castle wall segment", "castle_wall_04": "a stone castle wall segment",
    "castle_wall_05": "a stone castle wall segment", "castle_wall_06": "a stone castle wall segment",
}

CLASS = {
    "knight": "an armoured knight in a steel helm with a blue tabard",
    "paladin": "a holy paladin with a winged gold helm",
    "sorceress": "a sorceress in a purple hood holding a glowing gem",
    "barbarian": "a bare-chested barbarian with wild hair and a beard",
}


def stable_seed(path):
    h = 2166136261
    for ch in path:
        h = ((h ^ ord(ch)) * 16777619) & 0xFFFFFFFF
    return h % 1_000_000_000


def prompt_all(path):
    name = os.path.basename(path)
    stem = name[:-4]
    if path.startswith("art/tiles/"):
        if stem in TERRAIN:
            return (f"seamless top-down terrain texture, {TERRAIN[stem]}, painterly digital "
                    f"painting, fantasy strategy game art, soft even lighting, high detail"), False
        return None
    if path.startswith("art/troops/"):
        unit = stem.rsplit("_", 1)[0]
        return (f"a single {TROOP.get(unit, unit)}, full body, side view, centered, one "
                f"character only, painterly fantasy strategy game unit sprite, plain flat "
                f"neutral background, soft light, high detail"), True
    if path.startswith("art/villains/"):
        who = stem.rsplit("_", 1)[0].replace("_", " ")
        return (f"a single villain portrait bust, {who}, menacing, painterly fantasy "
                f"strategy game art, centered, plain dark background, dramatic light, "
                f"high detail"), True
    if path.startswith("art/sprites/"):
        anim = stem.rsplit("_", 1)[0]
        if anim == "hero_walk":
            return ("a single knight hero marching, full body, side view, centered, one "
                    "character only, painterly fantasy strategy game unit sprite, plain flat "
                    "neutral background, soft light, high detail"), True
        if anim == "boat":
            return ("a small wooden rowing boat with a striped sail, side view, centered, "
                    "painterly fantasy strategy game asset, plain flat neutral background, "
                    "high detail"), True
        return None
    if path.startswith("art/classes/"):
        return (f"a single {CLASS.get(stem, stem)} character portrait bust, painterly fantasy "
                f"strategy game art, centered, plain background, high detail"), True
    if path.startswith("art/objects/") or stem in OBJECT:
        desc = OBJECT.get(stem)
        if desc:
            return (f"a single {desc}, top-down, centered, painterly fantasy strategy game "
                    f"asset, plain flat neutral background, high detail"), True
    if stem in COMBAT:
        opaque = stem == "field_grass"
        return (f"a single {COMBAT[stem]}, centered, painterly fantasy strategy game asset, "
                f"plain flat neutral background, high detail"), not opaque
    return None


def frame_shift(w, h, rgba, k):
    out = bytearray(w * h * 4)
    for y in range(h):
        sy = y + k
        if 0 <= sy < h:
            out[y * w * 4:(y + 1) * w * 4] = rgba[sy * w * 4:(sy + 1) * w * 4]
    return bytes(out)


def compose_edges(tiles_dir):
    grass = read_png(os.path.join(tiles_dir, "grass.png"))[2]
    from modern_proof import W as EW, H as EH, outside_pred, smoothstep as ss, mix as mx
    for terrain, mapping in (("forest", SIDES), ("desert", SIDES),
                             ("mountain", SIDES), ("water", WATER_SIDES)):
        base = read_png(os.path.join(tiles_dir, terrain + ".png"))[2]
        for num, (kind, arg) in mapping.items():
            pred = outside_pred(kind, arg)
            out = bytearray(EW * EH * 4)
            for y in range(EH):
                for x in range(EW):
                    b = pred(x, y)
                    t = ss(-2.5, 2.5, b)
                    o = (y * EW + x) * 4
                    col = mx(grass[o:o + 3], base[o:o + 3], t)
                    if b < 0:
                        col = mx(col, (0.1, 0.15, 0.1), 0.35 * (1 - ss(-10, 0, b)))
                    elif b < 4:
                        col = mx(col, (1, 1, 1), 0.25 * (1 - b / 4))
                    out[o:o + 4] = bytes([int(col[0]), int(col[1]), int(col[2]), 255])
            write_png(os.path.join(tiles_dir, f"{terrain}_edge_{num}.png"), EW, EH, bytes(out))
    print("composed 48 terrain edges")


def resolve_rembg(arg):
    """Find the rembg executable: an explicit path, PATH, or the matte venv."""
    if arg and arg != "rembg":
        return arg if os.path.exists(arg) else shutil.which(arg)
    found = shutil.which("rembg")
    if found:
        return found
    fallback = os.path.expanduser("~/dev/ai/matte-venv/bin/rembg")
    return fallback if os.path.exists(fallback) else None


def need_rembg(args, category="", assets=None):
    if args.matte != "cli":
        return None
    found = resolve_rembg(getattr(args, "rembg", "rembg"))
    if found:
        args.rembg = found
        return found
    if category == "tiles" or (assets is not None and not any(
            a["path"].startswith(("art/troops/", "art/villains/", "art/sprites/",
                                  "art/classes/", "art/combat/", "art/objects/"))
            for a in assets)):
        return None
    sys.exit("rembg not found. Install it (pip install \"rembg[cpu]\" in "
             "~/dev/ai/matte-venv) or pass --rembg /full/path/to/rembg.")


def generate_sprite(args, path, positive, matte_wanted, w, h):
    matte = None
    graph = build_graph(args.ckpt, positive, NEG, stable_seed(path), w, h, matte)
    png = generate(args.host, graph)
    tmp = os.path.join(OUT, "_raw.png")
    with open(tmp, "wb") as fh:
        fh.write(png)
    src = tmp
    if matte_wanted and args.matte == "cli":
        cut = os.path.join(OUT, "_cut.png")
        subprocess.run([args.rembg, "i", tmp, cut], check=True)
        src = cut
    return read_png(src)[2]


def cmd_batch(args):
    with open(MANIFEST) as fh:
        assets = [a for a in json.load(fh)["assets"] if a["engine_used"]]
    if args.category:
        assets = [a for a in assets if a["path"].startswith("art/" + args.category + "/")]
    need_rembg(args, args.category, assets)
    os.makedirs(OUT, exist_ok=True)
    tiles_dir = os.path.join(OUT, "art", "tiles")
    os.makedirs(tiles_dir, exist_ok=True)

    done = 0
    failed = 0
    seen_groups = set()
    for asset in assets:
        path = asset["path"]
        if args.only and args.only not in path:
            continue
        name = os.path.basename(path)
        stem = name[:-4]
        group = None
        frames = 1
        if path.startswith(("art/troops/", "art/villains/", "art/sprites/")):
            group = stem.rsplit("_", 1)[0]
            frames = 4
        key = path if group is None else f"{os.path.dirname(path)}/{group}"
        if key in seen_groups:
            continue
        seen_groups.add(key)

        prompt = prompt_all(path)
        if prompt is None:
            continue
        positive, matte_wanted = prompt
        if args.limit and done >= args.limit:
            break
        outs = [os.path.join(OUT, path)] if frames == 1 else [
            os.path.join(OUT, os.path.dirname(path), f"{group}_{i:02d}.png") for i in range(4)]
        if args.resume and all(os.path.exists(o) for o in outs):
            print(f"skip      {path}")
            continue
        try:
            if group is None:
                print(f"gen       {path}", flush=True)
                rgba = generate_sprite(args, path, positive, matte_wanted, GEN_W, GEN_H)
                tw, th = asset["native"]["w"] * args.scale, asset["native"]["h"] * args.scale
                _, _, rgba = box_downscale(GEN_W, GEN_H, rgba, tw, th)
                rgba = grade(tw, th, rgba)
                write_png(outs[0], tw, th, rgba)
            else:
                print(f"gen       {group}  -> 4 frames", flush=True)
                rgba = generate_sprite(args, path, positive, matte_wanted, GEN_W, GEN_H)
                tw, th = asset["native"]["w"] * args.scale, asset["native"]["h"] * args.scale
                _, _, rgba = box_downscale(GEN_W, GEN_H, rgba, tw, th)
                for i, k in enumerate((0, 2, 0, 2)):
                    f = frame_shift(tw, th, rgba, k)
                    write_png(outs[i], tw, th, grade(tw, th, f))
            done += 1
        except Exception as exc:  # noqa: BLE001
            failed += 1
            print(f"  FAIL {path}: {exc}", file=sys.stderr, flush=True)
            continue

    if not args.category or args.category == "tiles":
        compose_edges(tiles_dir)
    print(f"batch done: {done} generations, {failed} failed")
    return 1 if failed else 0


def cmd_sheet(args):
    rows = []
    for root, _, files in os.walk(OUT):
        for f in sorted(files):
            if f.endswith(".png") and not f.startswith("_"):
                rel = os.path.relpath(os.path.join(root, f), OUT)
                rows.append((rel, rel.replace("\\", "/")))
    rows.sort()
    figs = "".join(
        f'<figure><img src="{src}"><figcaption>{rel}</figcaption></figure>'
        for rel, src in rows)
    html = ("<!doctype html><meta charset=utf-8><title>Open pack AI review</title>"
            "<style>body{background:#1b1a20;color:#ddd;font:13px system-ui;padding:2rem}"
            ".grid{display:flex;flex-wrap:wrap;gap:1rem}"
            "figure{margin:0;background:#2a2932;padding:.5rem;border-radius:8px}"
            "img{width:200px;image-rendering:auto;display:block;background:"
            "repeating-conic-gradient(#333 0 25%,#2a2a2a 0 50%) 0 0/16px 16px}"
            "figcaption{margin-top:.3rem;color:#99a;word-break:break-all;max-width:200px}"
            "</style><h1>Open pack — AI review</h1><div class=grid>" + figs + "</div>")
    path = os.path.join(OUT, "index.html")
    with open(path, "w") as fh:
        fh.write(html)
    print(f"{os.path.relpath(path, ROOT)}: {len(rows)} images")
    return 0


def cmd_one(args):
    with open(MANIFEST) as fh:
        manifest = json.load(fh)
    asset = next((a for a in manifest["assets"] if a["path"] == args.path), None)
    if asset is None:
        sys.exit(f"no such asset in manifest: {args.path}")
    positive, matte_wanted = prompt_for(args.path)
    mode = args.matte if matte_wanted else "none"
    if mode == "cli":
        need_rembg(args, assets=[asset])
    matte = None
    if mode == "node":
        info = http_get(args.host, "/object_info")
        cls = args.matte_node or find_matte(info)
        if cls:
            slot = matte_image_slot(info, cls)
            matte = (cls, matte_node_inputs(info, cls, ["8", 0]), slot)
    print(f"asset     {args.path}")
    print(f"positive  {positive}")
    print(f"matte     {mode}")

    graph = build_graph(args.ckpt, positive, NEG, args.seed, GEN_W, GEN_H, matte)
    png = generate(args.host, graph)
    os.makedirs(OUT, exist_ok=True)
    tmp = os.path.join(OUT, "_raw.png")
    with open(tmp, "wb") as fh:
        fh.write(png)
    src_file = tmp
    if mode == "cli":
        cut = os.path.join(OUT, "_cut.png")
        subprocess.run([args.rembg, "i", tmp, cut], check=True)
        src_file = cut
    w, h, rgba = read_png(src_file)
    tw = asset["native"]["w"] * args.scale
    th = asset["native"]["h"] * args.scale
    w, h, rgba = box_downscale(w, h, rgba, tw, th)
    rgba = grade(w, h, rgba)
    dst = os.path.join(OUT, args.path)
    write_png(dst, w, h, rgba)
    print(f"wrote     {os.path.relpath(dst, ROOT)}  ({w}x{h})")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--host", default="http://127.0.0.1:8188")
    ap.add_argument("--ckpt", default=CKPT)
    ap.add_argument("--scale", type=int, default=4)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--matte", choices=("cli", "node", "none"), default="cli",
                    help="how to cut out alpha (default: rembg CLI)")
    ap.add_argument("--rembg", default="rembg", help="path to the rembg executable")
    ap.add_argument("--matte-node", default="")
    sub = ap.add_subparsers(dest="command", required=True)
    sub.add_parser("probe").set_defaults(func=cmd_probe)
    p = sub.add_parser("one")
    p.add_argument("path")
    p.set_defaults(func=cmd_one)
    b = sub.add_parser("batch")
    b.add_argument("--category", default="", help="art/<category> only")
    b.add_argument("--only", default="", help="paths containing this substring")
    b.add_argument("--limit", type=int, default=0, help="stop after N groups")
    b.add_argument("--resume", action="store_true", help="skip existing outputs")
    b.set_defaults(func=cmd_batch)
    sub.add_parser("sheet").set_defaults(func=cmd_sheet)
    args = ap.parse_args()
    sys.exit(args.func(args))


if __name__ == "__main__":
    main()
