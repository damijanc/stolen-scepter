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
import math
import os
import shutil
import subprocess
import sys
import time
import traceback
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from upscale import read_png, write_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "tools", "hd_assets.json")
PROVENANCE = os.path.join(ROOT, "packs", "open", "provenance.json")
OUT = os.path.join(ROOT, "game-assets", "ai-out")

CKPT = "sd_xl_base_1.0.safetensors"
VERSION = "1.8"
GEN_W, GEN_H = 1024, 1024
SHEET_W, SHEET_H = 1344, 448
NEG_SHEET = ("text, watermark, labels, numbers, letters, border, frame, grid lines, table, "
             "different characters, inconsistent design, mismatched, overlap, cropped, "
             "blurry, low quality, deformed")
NEG = ("text, watermark, ui, border, frame, perspective, horizon, buildings, "
       "multiple figures, two characters, three characters, several, group, crowd, army, "
       "squad, party, lineup, concept art, character sheet, turnaround, reference sheet, "
       "collage, grid, split image, panel, clone, duplicate, mirror, reflection, "
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


def http_post_quiet(host, path, obj):
    body = json.dumps(obj).encode()
    req = urllib.request.Request(host + path, data=body,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=15) as r:
            r.read()
    except Exception:  # noqa: BLE001
        pass


def clear_queue(host):
    """Stop any running prompt and drop the pending queue.

    Ctrl-C kills this script, not the ComfyUI prompt: it keeps generating and
    can save over the next run's files. Clear before a batch starts.
    """
    http_post_quiet(host, "/interrupt", {})
    http_post_quiet(host, "/queue", {"clear": True})
    try:
        q = http_get(host, "/queue")
        running = len(q.get("queue_running", []))
        pending = len(q.get("queue_pending", []))
        print(f"queue cleared (running {running}, pending {pending})", flush=True)
    except Exception:  # noqa: BLE001
        pass


def find_matte(info):
    for cls, spec in info.items():
        blob = (cls + " " + str(spec.get("display_name", ""))).lower()
        if "rmbg" in blob or ("remove" in blob and "background" in blob):
            return cls
    return None


def cmd_probe(args):
    print(f"gen_ai version {VERSION}")
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
    print("--- IP-Adapter nodes ---")
    for cls, spec in sorted(info.items()):
        if "ipadapter" in cls.lower():
            req = spec.get("input", {}).get("required", {})
            fields = {}
            for k, v in req.items():
                t = v[0] if v else None
                if isinstance(t, list):
                    fields[k] = f"choice[{len(t)}] e.g. {t[:3]}"
                else:
                    fields[k] = t
            print(f"{cls}: in={fields} out={spec.get('output_name') or spec.get('output')}")
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


def upload_image(host, path):
    """POST a PNG to ComfyUI's input dir; returns the name LoadImage expects."""
    data = open(path, "rb").read()
    boundary = "----ob-boundary-7f3a"
    body = (f"--{boundary}\r\n"
            f'Content-Disposition: form-data; name="image"; filename="ob_frame.png"\r\n'
            f"Content-Type: image/png\r\n\r\n").encode() + data + b"\r\n"
    body += (f"--{boundary}\r\n"
             f'Content-Disposition: form-data; name="overwrite"\r\n\r\n1\r\n').encode()
    body += f"--{boundary}--\r\n".encode()
    req = urllib.request.Request(host + "/upload/image", data=body,
                                 headers={"Content-Type":
                                          f"multipart/form-data; boundary={boundary}"})
    with urllib.request.urlopen(req, timeout=60) as r:
        doc = json.loads(r.read())
    return doc.get("name", "ob_frame.png")


def build_img2img(ckpt, positive, negative, seed, image_name, denoise):
    return {
        "4": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": ckpt}},
        "20": {"class_type": "LoadImage", "inputs": {"image": image_name}},
        "21": {"class_type": "VAEEncode", "inputs": {"pixels": ["20", 0], "vae": ["4", 2]}},
        "6": {"class_type": "CLIPTextEncode", "inputs": {"text": positive, "clip": ["4", 1]}},
        "7": {"class_type": "CLIPTextEncode", "inputs": {"text": negative, "clip": ["4", 1]}},
        "3": {"class_type": "KSampler",
              "inputs": {"seed": seed, "steps": 30, "cfg": 6.0,
                         "sampler_name": "dpmpp_2m", "scheduler": "karras",
                         "denoise": denoise, "model": ["4", 0],
                         "positive": ["6", 0], "negative": ["7", 0],
                         "latent_image": ["21", 0]}},
        "8": {"class_type": "VAEDecode", "inputs": {"samples": ["3", 0], "vae": ["4", 2]}},
        "10": {"class_type": "SaveImage",
               "inputs": {"images": ["8", 0], "filename_prefix": "ob"}},
    }


def build_ipadapter_img2img(ckpt, positive, negative, seed, image_name, denoise,
                            preset, weight):
    return {
        "4": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": ckpt}},
        "20": {"class_type": "LoadImage", "inputs": {"image": image_name}},
        "30": {"class_type": "IPAdapterUnifiedLoader",
               "inputs": {"model": ["4", 0], "preset": preset}},
        "31": {"class_type": "IPAdapter",
               "inputs": {"model": ["30", 0], "ipadapter": ["30", 1], "image": ["20", 0],
                          "weight": weight, "start_at": 0.0, "end_at": 1.0,
                          "weight_type": "standard"}},
        "21": {"class_type": "VAEEncode", "inputs": {"pixels": ["20", 0], "vae": ["4", 2]}},
        "6": {"class_type": "CLIPTextEncode", "inputs": {"text": positive, "clip": ["4", 1]}},
        "7": {"class_type": "CLIPTextEncode", "inputs": {"text": negative, "clip": ["4", 1]}},
        "3": {"class_type": "KSampler",
              "inputs": {"seed": seed, "steps": 30, "cfg": 6.0,
                         "sampler_name": "dpmpp_2m", "scheduler": "karras",
                         "denoise": denoise, "model": ["31", 0],
                         "positive": ["6", 0], "negative": ["7", 0],
                         "latent_image": ["21", 0]}},
        "8": {"class_type": "VAEDecode", "inputs": {"samples": ["3", 0], "vae": ["4", 2]}},
        "10": {"class_type": "SaveImage",
               "inputs": {"images": ["8", 0], "filename_prefix": "ob"}},
    }


def resolve_ip_preset(info, want):
    choices = (info.get("IPAdapterUnifiedLoader", {}).get("input", {})
               .get("required", {}).get("preset", [[]])[0])
    if want and want in choices:
        return want
    for c in choices:
        if "PLUS" in c and "FACE" not in c:
            return c
    for c in choices:
        if "STANDARD" in c:
            return c
    return choices[0] if choices else "STANDARD (medium strength)"


def run_graph_matted(args, graph, matte_wanted):
    png = generate(args.host, graph)
    os.makedirs(OUT, exist_ok=True)
    tmp = os.path.join(OUT, "_raw.png")
    with open(tmp, "wb") as fh:
        fh.write(png)
    src = tmp
    if matte_wanted and args.matte == "cli":
        cut = os.path.join(OUT, "_cut.png")
        subprocess.run([args.rembg, "i", tmp, cut], check=True)
        src = cut
    return read_png(src)[2]


def matte_file(args, raw_path, tag):
    cut = os.path.join(OUT, f"_cut_{tag}.png")
    subprocess.run([args.rembg, "i", raw_path, cut], check=True)
    return read_png(cut)[2]


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


def sheet_prompt(path):
    """A 4-pose horizontal sprite sheet for an animated unit, or None."""
    stem = os.path.basename(path)[:-4]
    if path.startswith("art/troops/"):
        unit = stem.rsplit("_", 1)[0]
        desc = TROOP.get(unit, unit)
    elif path.startswith("art/sprites/"):
        if stem.rsplit("_", 1)[0] != "hero_walk":
            return None
        desc = "knight hero"
    else:
        return None
    return (f"animation sprite sheet: the same {desc} in 4 sequential idle-animation frames, "
            f"arranged left to right in one horizontal row, identical character in every "
            f"frame, full body, side view, evenly spaced, plain flat neutral background, "
            f"painterly fantasy strategy game art, high detail")


def stable_seed(path):
    h = 2166136261
    for ch in path:
        h = ((h ^ ord(ch)) * 16777619) & 0xFFFFFFFF
    return h % 1_000_000_000


UI_FULL = {
    "backdrop_town", "backdrop_castle", "backdrop_plains", "backdrop_forest",
    "backdrop_hillcave", "backdrop_dungeon", "splash_title", "class_select_picker",
    "hud_bar_strip", "hud_puzzle_grid", "end_grass", "end_carpet",
    "end_win_screen", "end_lose_screen", "chrome_overworld",
}
UI_DESC = {
    "backdrop_town": "a medieval fantasy town, distant view",
    "backdrop_castle": "a medieval castle on a hill, distant view",
    "backdrop_plains": "rolling grassy plains, distant view",
    "backdrop_forest": "a dense green forest, distant view",
    "backdrop_hillcave": "a rocky cave entrance in green hills",
    "backdrop_dungeon": "a dark stone dungeon interior lit by torches",
    "splash_logo": "an ornate fantasy game emblem in gold",
    "splash_title": "an epic fantasy strategy game title: a great castle under a "
                    "dramatic sky",
    "class_select_picker": "an ornate empty stone frame for character portraits",
    "class_select_highlight": "a glowing golden selection frame",
    "hud_bar_strip": "an ornate horizontal decorative stone bar",
    "hud_contract_silhouette": "a hooded villain silhouette",
    "hud_siege_silhouette": "a catapult siege engine silhouette",
    "hud_magic_silhouette": "a spellbook silhouette",
    "hud_puzzle_grid": "a small parchment map grid",
    "hud_gold_purse": "a small bag of gold coins",
    "end_grass": "a top-down green grass tile",
    "end_carpet": "a top-down red carpet tile",
    "end_hero": "a small knight hero standing on a tile",
    "end_win_screen": "a triumphant victory scene with a hero on a throne",
    "end_lose_screen": "a sombre defeat scene in a dark throne room",
    "chrome_overworld": "an ornate stone game interface frame border with an empty center",
}


def ui_prompt(stem):
    if stem.startswith("inventory_artifact_"):
        return f"an inventory icon of a {stem[len('inventory_artifact_'):]}, painterly", True
    if stem.startswith("inventory_zone_"):
        return (f"a rolled parchment map icon of {stem[len('inventory_zone_'):]}, painterly"), True
    if stem.startswith(("hud_siege_", "hud_magic_")):
        return f"{stem}, a small game icon, painterly", True
    if stem in UI_DESC:
        return (f"{UI_DESC[stem]}, painterly digital painting, fantasy strategy game art, "
                f"high detail"), stem not in UI_FULL
    return None


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
        return (f"a single {TROOP.get(unit, unit)}, one lone figure, full body, side view "
                f"facing right, centered, large in frame, isolated game asset sprite, one character only, "
                f"painterly fantasy strategy game unit, plain flat neutral background, "
                f"crisp, high detail"), True
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
        return (f"{CLASS.get(stem, stem)}, character portrait bust, painterly fantasy "
                f"strategy game art, centered, plain background, high detail"), True
    if path.startswith("art/objects/") or stem in OBJECT:
        desc = OBJECT.get(stem)
        if desc:
            return (f"a single {desc}, top-down, centered, painterly fantasy strategy game "
                    f"asset, plain flat neutral background, high detail"), True
    if stem in COMBAT:
        opaque = stem == "field_grass"
        return (f"{COMBAT[stem]}, centered, painterly fantasy strategy game asset, "
                f"plain flat neutral background, high detail"), not opaque
    if path.startswith("art/ui/"):
        return ui_prompt(stem)
    return None


def frame_shift(w, h, rgba, k):
    out = bytearray(w * h * 4)
    for y in range(h):
        sy = y + k
        if 0 <= sy < h:
            out[y * w * 4:(y + 1) * w * 4] = rgba[sy * w * 4:(sy + 1) * w * 4]
    return bytes(out)


def alpha_components(w, h, rgba):
    """Connected blobs of opaque pixels, largest first."""
    seen = bytearray(w * h)
    comps = []
    for i in range(w * h):
        if seen[i] or rgba[i * 4 + 3] < 16:
            continue
        stack = [i]
        seen[i] = 1
        blob = []
        while stack:
            j = stack.pop()
            blob.append(j)
            x, y = j % w, j // w
            for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if 0 <= nx < w and 0 <= ny < h:
                    k = ny * w + nx
                    if not seen[k] and rgba[k * 4 + 3] >= 16:
                        seen[k] = 1
                        stack.append(k)
        comps.append(blob)
    comps.sort(key=len, reverse=True)
    return comps


def bilinear_resize(w, h, rgba, nw, nh):
    out = bytearray(nw * nh * 4)
    for y in range(nh):
        sy = (y + 0.5) * h / nh - 0.5
        y0 = math.floor(sy)
        ty = min(1.0, max(0.0, sy - y0))
        y0 = max(0, min(h - 1, y0))
        y1 = min(h - 1, y0 + 1)
        for x in range(nw):
            sx = (x + 0.5) * w / nw - 0.5
            x0 = math.floor(sx)
            tx = min(1.0, max(0.0, sx - x0))
            x0 = max(0, min(w - 1, x0))
            x1 = min(w - 1, x0 + 1)
            base = (y * nw + x) * 4
            for c in range(4):
                a = rgba[(y0 * w + x0) * 4 + c]
                b = rgba[(y0 * w + x1) * 4 + c]
                d = rgba[(y1 * w + x0) * 4 + c]
                e = rgba[(y1 * w + x1) * 4 + c]
                v = (a * (1 - tx) + b * tx) * (1 - ty) + (d * (1 - tx) + e * tx) * ty
                out[base + c] = min(255, max(0, int(v + 0.5)))
    return nw, nh, bytes(out)


def crop_fit(w, h, rgba, x0, y0, x1, y1, tw, th):
    """Crop a source box and letterbox it into a tw x th cell, aspect kept."""
    bw, bh = x1 - x0 + 1, y1 - y0 + 1
    crop = bytearray(bw * bh * 4)
    for y in range(bh):
        s = ((y0 + y) * w + x0) * 4
        crop[y * bw * 4:(y + 1) * bw * 4] = rgba[s:s + bw * 4]
    scale = min(tw / bw, th / bh)
    nw, nh = max(1, min(tw, int(bw * scale))), max(1, min(th, int(bh * scale)))
    if (nw, nh) != (bw, bh):
        _, _, crop = bilinear_resize(bw, bh, bytes(crop), nw, nh)
    canvas = bytearray(tw * th * 4)
    ox, oy = (tw - nw) // 2, (th - nh) // 2
    for y in range(nh):
        s = y * nw * 4
        d = ((oy + y) * tw + ox) * 4
        canvas[d:d + nw * 4] = crop[s:s + nw * 4]
    return bytes(canvas)


def fit_sprite(w, h, rgba, tw, th):
    """Keep the largest blob, crop to it, and fit it into a tw x th cell."""
    comps = alpha_components(w, h, rgba)
    if not comps:
        return tw, th, bytes(tw * th * 4)
    blob = comps[0]
    xs = [j % w for j in blob]
    ys = [j // w for j in blob]
    return tw, th, crop_fit(w, h, rgba, min(xs), min(ys), max(xs), max(ys), tw, th)


# Combat units loop a 4-frame IDLE animation (combat.h: `frame // 0..3 idle
# anim`, advanced by `combat_tick_anim`). Movement is instant and attacks are a
# splat overlay, so there is no walk/attack frame split -- it is one resting
# cycle. The hero is the exception: `hero_walk` plays while he moves on the map.
IDLE_POSES = [
    "standing at rest with the weapon lowered at the side",
    "raising the weapon, arm midway up, torso turning",
    "weapon raised high overhead, ready to strike",
    "lowering the weapon back down, returning to rest",
]
WALK_POSES = [
    "standing with feet together",
    "mid-stride with the left leg forward, weapon raised",
    "standing with feet together, body lifted",
    "mid-stride with the right leg forward, weapon lowered",
]


def frame_prompt(path, i):
    stem = os.path.basename(path)[:-4]
    if path.startswith("art/troops/"):
        desc = TROOP.get(stem.rsplit("_", 1)[0], stem)
        poses = IDLE_POSES
    elif path.startswith("art/sprites/") and stem.rsplit("_", 1)[0] == "hero_walk":
        desc = "knight hero"
        poses = WALK_POSES
    else:
        return None
    return (f"a single {desc}, {poses[i]}, full body, side view facing right, centered, "
            f"large in frame, one character only, consistent character design, painterly "
            f"fantasy strategy game unit, plain flat neutral background, crisp, high "
            f"detail"), True


def idle_frames(tw, th, rgba):
    """Four distinct idle frames from one sprite: a small circular bob.

    The bob amplitude scales with the cell so it stays visible as the art
    resolution grows.
    """
    amp = max(1, th // 68)
    out = []
    for dx, dy in [(0, 0), (amp, -amp), (0, -2 * amp), (-amp, -amp)]:
        canvas = bytearray(tw * th * 4)
        for y in range(th):
            sy = y + dy
            if not (0 <= sy < th):
                continue
            row = sy * tw * 4
            if dx == 0:
                canvas[y * tw * 4:(y + 1) * tw * 4] = rgba[row:row + tw * 4]
            else:
                for x in range(tw):
                    sx = x + dx
                    if 0 <= sx < tw:
                        canvas[(y * tw + x) * 4:(y * tw + x) * 4 + 4] = \
                            rgba[row + sx * 4:row + sx * 4 + 4]
        out.append(bytes(canvas))
    return out


def try_sheet(args, path, tw, th):
    sp = sheet_prompt(path)
    if not sp:
        return None
    seed = stable_seed(path)
    for attempt in range(args.tries + 1):
        rgba = generate_sprite(args, path, sp, True, SHEET_W, SHEET_H, seed + attempt, NEG_SHEET)
        comps = alpha_components(SHEET_W, SHEET_H, rgba)
        total = sum(len(c) for c in comps) or 1
        big = [c for c in comps if len(c) > 0.03 * total]
        if len(big) >= 4:
            big = sorted(big, key=lambda c: min(j % SHEET_W for j in c))[:4]
            frames = []
            for blob in big:
                xs = [j % SHEET_W for j in blob]
                ys = [j // SHEET_W for j in blob]
                frames.append(crop_fit(SHEET_W, SHEET_H, rgba, min(xs), min(ys),
                                       max(xs), max(ys), tw, th))
            return frames
        print(f"  sheet had {len(big)} figures, retry {attempt + 1}", flush=True)
    return None


def render_group_frames(args, path, tw, th):
    """4 frames: single sprite + img2img poses (default), AI sheet, or a bob."""
    if getattr(args, "sheet", False):
        frames = try_sheet(args, path, tw, th)
        if frames:
            return frames
    if frame_prompt(path, 0) is None:
        p = prompt_all(path)
        _, _, rgba = render_asset(args, path, p[0], p[1], tw, th)
        return idle_frames(tw, th, rgba)
    seed = stable_seed(path)
    os.makedirs(OUT, exist_ok=True)
    pos0, _ = frame_prompt(path, 0)
    raw0 = generate_sprite(args, path, pos0, False, GEN_W, GEN_H, seed)
    init = os.path.join(OUT, "_init.png")
    write_png(init, GEN_W, GEN_H, raw0)
    frames = [fit_sprite(GEN_W, GEN_H, matte_file(args, init, "0"), tw, th)[2]]
    denoise = getattr(args, "denoise", 0.0)
    if denoise > 0:
        try:
            name = upload_image(args.host, init)
            info = http_get(args.host, "/object_info")
            for i in range(1, 4):
                pos, _ = frame_prompt(path, i)
                if getattr(args, "ipadapter", False):
                    preset = resolve_ip_preset(info, getattr(args, "ip_preset", ""))
                    graph = build_ipadapter_img2img(
                        args.ckpt, pos, NEG, seed, name, denoise, preset,
                        getattr(args, "ip_weight", 0.8))
                else:
                    graph = build_img2img(args.ckpt, pos, NEG, seed, name, denoise)
                png = generate(args.host, graph)
                raw = os.path.join(OUT, f"_raw_{i}.png")
                with open(raw, "wb") as fh:
                    fh.write(png)
                frames.append(fit_sprite(GEN_W, GEN_H, matte_file(args, raw, str(i)), tw, th)[2])
            return frames
        except Exception as exc:  # noqa: BLE001
            print(f"  img2img failed ({exc}); using independent frames", flush=True)
    for i in range(1, 4):
        pos, _ = frame_prompt(path, i)
        rgba = generate_sprite(args, path, pos, False, GEN_W, GEN_H, seed)
        raw = os.path.join(OUT, f"_raw_{i}.png")
        write_png(raw, GEN_W, GEN_H, rgba)
        frames.append(fit_sprite(GEN_W, GEN_H, matte_file(args, raw, str(i)), tw, th)[2])
    return frames


def gen_size(tw, th):
    """A generation size matching the target aspect, multiples of 64, <= 1536."""
    aspect = tw / th
    base = 1024
    if aspect >= 1:
        w, h = base, int(base / aspect)
    else:
        w, h = int(base * aspect), base
    w = max(256, (w // 64) * 64)
    h = max(256, (h // 64) * 64)
    return min(w, 1536), min(h, 1536)


def render_asset(args, path, positive, matte_wanted, tw, th, base_seed=None):
    """Generate, and for sprites retry until one blob then frame it in the cell."""
    seed = stable_seed(path) if base_seed is None else base_seed
    gw, gh = (GEN_W, GEN_H) if matte_wanted else gen_size(tw, th)
    last = None
    for attempt in range(args.tries + 1):
        rgba = generate_sprite(args, path, positive, matte_wanted, gw, gh, seed + attempt)
        if not matte_wanted:
            _, _, scaled = box_downscale(gw, gh, rgba, tw, th)
            return tw, th, scaled
        comps = alpha_components(gw, gh, rgba)
        total = sum(len(c) for c in comps) or 1
        big = [c for c in comps if len(c) > 0.05 * total]
        last = rgba
        if len(big) <= 1:
            break
        print(f"  {os.path.basename(path)}: {len(big)} figures, retry {attempt + 1}")
    return fit_sprite(gw, gh, last, tw, th)


CASTLE = {"castle_tl": (0, 0), "castle_br": (1, 0), "castle_tr": (2, 0),
          "castle_ml": (0, 1), "castle_gate": (1, 1), "castle_mr": (2, 1)}


def render_castle(args, scale):
    """One 3x2 castle image, sliced into the six aligned, opaque pieces."""
    cw, ch = 48 * scale * 3, 34 * scale * 2
    gw, gh = gen_size(cw, ch)
    positive = ("a medieval fantasy stone castle with a central gate and two side towers, "
                "front view, centered, standing on grassy ground, painterly digital "
                "painting, fantasy strategy game art, warm light, high detail")
    rgba = generate_sprite(args, "art/tiles/castle_gate.png", positive, False, gw, gh,
                           stable_seed("castle"))
    _, _, full = box_downscale(gw, gh, rgba, cw, ch)
    tw, th = cw // 3, ch // 2
    tiles = {}
    for name, (cx, cy) in CASTLE.items():
        t = bytearray()
        for y in range(cy * th, (cy + 1) * th):
            s = (y * cw + cx * tw) * 4
            t += full[s:s + tw * 4]
        tiles[name] = bytes(t)
    return tiles, tw, th


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


def generate_sprite(args, path, positive, matte_wanted, w, h, seed, neg=NEG):
    graph = build_graph(args.ckpt, positive, neg, seed, w, h, None)
    return run_graph_matted(args, graph, matte_wanted)


def cmd_batch(args):
    with open(MANIFEST) as fh:
        assets = [a for a in json.load(fh)["assets"] if a["engine_used"]]
    if args.category:
        assets = [a for a in assets if a["path"].startswith("art/" + args.category + "/")]
    print(f"gen_ai version {VERSION}, scale {args.scale}, gen {GEN_W}x{GEN_H}", flush=True)
    if not getattr(args, "no_clear", False):
        clear_queue(args.host)
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

        if stem in CASTLE:
            if "castle" in seen_groups:
                continue
            seen_groups.add("castle")
            print("gen       castle (3x2) -> 6 pieces", flush=True)
            try:
                tiles, ctw, cth = render_castle(args, args.scale)
                for pname, px in tiles.items():
                    write_png(os.path.join(OUT, "art", "tiles", pname + ".png"),
                              ctw, cth, grade(ctw, cth, px))
                done += 1
            except Exception as exc:  # noqa: BLE001
                failed += 1
                print(f"  FAIL castle: {exc}", file=sys.stderr, flush=True)
            continue

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
            tw = asset["native"]["w"] * args.scale
            th = asset["native"]["h"] * args.scale
            if group is None:
                print(f"gen       {path}", flush=True)
                tw, th, rgba = render_asset(args, path, positive, matte_wanted, tw, th)
                write_png(outs[0], tw, th, grade(tw, th, rgba))
            else:
                print(f"gen       {group}  -> 4 frames", flush=True)
                frames = render_group_frames(args, path, tw, th)
                for i, f in enumerate(frames):
                    write_png(outs[i], tw, th, grade(tw, th, f))
            done += 1
        except Exception as exc:  # noqa: BLE001
            failed += 1
            print(f"  FAIL {path}: {exc}", file=sys.stderr, flush=True)
            traceback.print_exc()
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
    mode = "cli" if (matte_wanted and args.matte != "none") else "none"
    if mode == "cli":
        need_rembg(args, assets=[asset])
    print(f"asset     {args.path}")
    print(f"positive  {positive}")
    print(f"matte     {mode}")

    os.makedirs(OUT, exist_ok=True)
    tw = asset["native"]["w"] * args.scale
    th = asset["native"]["h"] * args.scale
    w, h, rgba = render_asset(args, args.path, positive, matte_wanted, tw, th, args.seed)
    rgba = grade(w, h, rgba)
    dst = os.path.join(OUT, args.path)
    write_png(dst, w, h, rgba)
    print(f"wrote     {os.path.relpath(dst, ROOT)}  ({w}x{h})")
    return 0


def main():
    global OUT
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--host", default="http://127.0.0.1:8188")
    ap.add_argument("--ckpt", default=CKPT)
    ap.add_argument("--scale", type=int, default=6, help="art pixel multiple (6 -> 288x204)")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--tries", type=int, default=2,
                    help="retries when a sprite sheet has the wrong figure count")
    ap.add_argument("--denoise", type=float, default=0.45,
                    help="img2img strength for animation frames (0 disables)")
    ap.add_argument("--ipadapter", action="store_true",
                    help="condition animation frames on frame 0 via IP-Adapter")
    ap.add_argument("--ip-weight", type=float, default=0.8)
    ap.add_argument("--ip-preset", default="", help="IPAdapterUnifiedLoader preset")
    ap.add_argument("--matte", choices=("cli", "node", "none"), default="cli",
                    help="how to cut out alpha (default: rembg CLI)")
    ap.add_argument("--rembg", default="rembg", help="path to the rembg executable")
    ap.add_argument("--matte-node", default="")
    ap.add_argument("--out", default=OUT, help="output directory")
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
    b.add_argument("--sheet", action="store_true",
                   help="try an AI sprite sheet for animated units (experimental)")
    b.add_argument("--no-clear", action="store_true",
                   help="do not interrupt/clear the ComfyUI queue first")
    b.set_defaults(func=cmd_batch)
    sub.add_parser("sheet").set_defaults(func=cmd_sheet)
    args = ap.parse_args()
    OUT = os.path.abspath(args.out)
    sys.exit(args.func(args))


if __name__ == "__main__":
    main()
