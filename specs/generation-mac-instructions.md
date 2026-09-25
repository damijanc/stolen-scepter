# Generation Mac — Operating Instructions

For the Mac that runs ComfyUI. The repo's main copy (with the original art dump)
is elsewhere; this machine only needs the repo clone, ComfyUI, and rembg.

Paths below assume the repo is at `~/Development/king-bounty` and ComfyUI at
`~/dev/ai/ComfyUI` — adjust if yours differ. See also `~/dev/ai/ai-art-setup.md`
(one-time setup log).

---

## Every session

### 1. Start ComfyUI (one terminal, leave running)

```sh
source ~/dev/ai/ComfyUI/venv/bin/activate
cd ~/dev/ai/ComfyUI
PYTORCH_ENABLE_MPS_FALLBACK=1 python main.py
```

Wait for `To see the GUI go to: http://127.0.0.1:8188`. Leave this terminal.

### 2. Update the repo (second terminal)

```sh
cd ~/Development/king-bounty
git pull
```

### 3. Confirm the server and tools

```sh
python3 tools/gen_ai.py probe
```

Expect (near the top): `gen_ai version <latest>`,
`registration: spec (208 slots in tools/asset_layout.json)`, your
`checkpoints:` list, and `prompt endpoint reachable: True`. If it says
connection refused, ComfyUI isn't running (step 1). If it says
`registration: fill (no tools/asset_layout.json)`, `git pull` again: the
layout file is committed and needed for sprite placement.

---

## One-time: CLIP-Vision encoder (required for IP-Adapter)

IP-Adapter needs a CLIP-ViT-H encoder. Check first:

```sh
ls -lh ~/dev/ai/ComfyUI/models/clip_vision
```

If it does not contain `CLIP-ViT-H-14-laion2B-s32B-b79K.safetensors` (~2.5 GB):

```sh
source ~/dev/ai/ComfyUI/venv/bin/activate
hf download h94/IP-Adapter models/image_encoder/model.safetensors --local-dir /tmp/clip
mkdir -p ~/dev/ai/ComfyUI/models/clip_vision
mv /tmp/clip/models/image_encoder/model.safetensors \
   ~/dev/ai/ComfyUI/models/clip_vision/CLIP-ViT-H-14-laion2B-s32B-b79K.safetensors
ls -lh ~/dev/ai/ComfyUI/models/clip_vision
```

Then **restart ComfyUI** (Ctrl-C in terminal 1, run the start command again).

Also confirm the IP-Adapter model file:

```sh
ls -lh ~/dev/ai/ComfyUI/models/ipadapter
# expect ip-adapter-plus_sdxl_vit-h.safetensors
```

---

## Generate troops (the current focus)

Run in the second terminal. `rembg` lives in its own venv:

```sh
cd ~/Development/king-bounty

# one unit first, to check quality
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py \
  --ipadapter --denoise 0.6 batch --category troops --only archers

python3 tools/gen_ai.py sheet && open game-assets/ai-out/index.html
```

Look for: **one** archer per frame, same character through the four frames,
transparent background, ~288×204, and the four beats of the archer's own
action in order: full draw → just loosed (no arrow) → bow pointed down → arrow
nocked. Each troop's four frames are its attack loop, not a generic idle; the
beats per unit are `TROOP_POSES` in `tools/gen_ai.py`.

If the frames all look like frame 0 (img2img cannot move far from it), let
IP-Adapter carry the identity alone and draw each pose fresh:

```sh
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py \
  --ipadapter --denoise 1.0 batch --category troops --only archers
```

At `--denoise 1.0` frame 0's pixels are ignored and only its look (via
IP-Adapter) carries over, so the pose follows the prompt. Needs the CLIP-Vision
encoder above.

Placement (`--registration spec`, the default): each unit is scaled once for
all four frames, to the original's height in the cell, and stands on the
original's ground line with its feet where the original's were. So units no
longer all fill the cell: a peasant is shorter than a dragon, and the four
frames do not change size. `--registration fill` restores the old
stretch-to-cell behaviour for comparison. Nothing about ComfyUI changes; this
happens after matting.

Tune if needed (flags go **before** `batch`):

```sh
# identity drifting? anchor it harder
python3 tools/gen_ai.py --ipadapter --ip-weight 1.0 ... batch --category troops --only archers
# pose too static? push denoise
python3 tools/gen_ai.py --ipadapter --denoise 0.7 ... batch --category troops --only archers
# pose too wild? pull it back
python3 tools/gen_ai.py --ipadapter --denoise 0.5 ... batch --category troops --only archers
# compare runs side by side
... batch --category troops --only archers --out /tmp/run-a
```

When archers look right, run the whole set (resumable; safe to stop):

```sh
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py \
  --ipadapter --denoise 0.6 batch --category troops --resume 2>&1 | tee -a ~/ai-batch.log
```

Then the other categories (same pattern):

```sh
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py --ipadapter batch --category villains --resume 2>&1 | tee -a ~/ai-batch.log
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py --ipadapter batch --category classes  --resume 2>&1 | tee -a ~/ai-batch.log
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py batch --category tiles 2>&1 | tee -a ~/ai-batch.log
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py batch --category combat --resume 2>&1 | tee -a ~/ai-batch.log
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py batch --category ui     --resume 2>&1 | tee -a ~/ai-batch.log
python3 tools/gen_ai.py sheet
```

Results land in `game-assets/ai-out/` (this Mac only). Nothing is committed.

---

## Rules during a run

- **One pipeline at a time.** Two runs share `game-assets/ai-out/…` filenames;
  the last writer wins.
- **Ctrl-C stops the script, not ComfyUI's prompt.** The next `batch` clears the
  queue automatically (`--no-clear` disables that). By hand: ComfyUI **Queue →
  Interrupt / Clear Queue**.
- Use `--resume` so you can stop and continue; failed items are skipped and
  reported (`N generations, M failed`).
- Keep the AI output on this Mac; report back rather than committing.

---

## When something fails

Paste the last ~30 lines of `~/ai-batch.log` (or the console), which now
includes a full Python traceback per failed item, plus the `probe` output.

Common ones:

| Symptom | Fix |
|---|---|
| `connection refused` | ComfyUI not running |
| `ClipVision model not found` | § one-time CLIP-Vision above, then restart ComfyUI |
| `rembg not found` | run with `PATH="$HOME/dev/ai/matte-venv/bin:$PATH"` or `--rembg /full/path` |
| `No onnxruntime backend` | `pip install "rembg[cpu]"` in `~/dev/ai/matte-venv` |
| `byte must be in range(0,256)` | old script; `git pull` to the latest version |
| black/garbled images | restart ComfyUI with `--force-fp16` |
| machine swapping hard | restart ComfyUI with `--lowvram` |

---

## Reporting back

Send: the `probe` version line, one generated unit's result (or how it looks),
and any traceback/`FAIL` lines. Don't commit — the primary Mac handles
promotion into `packs/open/art` and the built pack.
