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
mv /tmp/clip/models/image_encoder/model.safetensors ~/dev/ai/ComfyUI/models/clip_vision/CLIP-ViT-H-14-laion2B-s32B-b79K.safetensors
ls -lh ~/dev/ai/ComfyUI/models/clip_vision
```

Then **restart ComfyUI** (Ctrl-C in terminal 1, run the start command again).

Also confirm the IP-Adapter model file:

```sh
ls -lh ~/dev/ai/ComfyUI/models/ipadapter
# expect ip-adapter-plus_sdxl_vit-h.safetensors
```

---

## One-time: ControlNet OpenPose (required for `--controlnet`)

Troop poses and facing come from pose skeletons the script draws itself
(`tools/pose_rig.py`); a ControlNet makes SDXL follow them. It needs one model
file and no custom nodes (`ControlNetLoader` / `ControlNetApplyAdvanced` are
built into ComfyUI; `comfyui_controlnet_aux` is NOT needed -- nothing is
detected from images).

- Model: Hugging Face repo `xinsir/controlnet-openpose-sdxl-1.0`, file
  `diffusion_pytorch_model.safetensors` (confirm the name in the repo).
- Put it at `~/dev/ai/ComfyUI/models/controlnet/controlnet-openpose-sdxl-1.0.safetensors`
  (that is the name the script loads; another name needs `--cn-model <name>`).
- Restart ComfyUI.

`probe` then prints `controlnet: ready`. `MISSING ...` means the file is not
where ComfyUI looks, or has another name.

---

## Generate troops (the current focus)

Run in the second terminal. `rembg` lives in its own venv.

Optional, no ComfyUI needed: look at the skeletons first. They land in
`game-assets/ai-out/poses/` and show on the review sheet.

```sh
python3 tools/gen_ai.py poses --only archers
```

Then one unit, to check quality:

```sh
cd ~/Development/king-bounty
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py --controlnet --ipadapter batch --category troops --only archers

python3 tools/gen_ai.py sheet && open game-assets/ai-out/index.html
```

The log must print `gen_ai version 1.15` and `controlnet on`. A missing model
stops the run with the fix instead of quietly falling back.

Look for: **one** archer per frame, **facing right**, same character through
the four frames, transparent background, ~288×204, and the four beats of the
archer's own action in order: full draw → just loosed (no arrow) → bow pointed
down → arrow nocked. Each troop's four frames are its attack loop, not a
generic idle; the beats per unit are `TROOP_POSES` in `tools/gen_ai.py`, and
the matching skeletons are `UNIT_POSES` in `tools/pose_rig.py`.

How the frames are made (`--controlnet`): every frame is a fresh generation
drawn to its own skeleton, which fixes the pose and the facing. Frames 1-3 also
get frame 0 through IP-Adapter in `style transfer` mode, which carries the
character's look but not its pose. `--denoise` does not apply here.

Not posed (no human skeleton): cavalry, nomads, wolves, dragons. They are
prompt-driven (fresh frames at the default `--denoise 1.0`, look carried by
IP-Adapter), so check their facing; flip any that came out facing
left with `--mirror`, e.g. `--mirror wolves,dragons_02` (path substrings),
and re-run just that unit.

Placement (`--registration spec`, the default): each unit is scaled once for
all four frames, to the original's height in the cell, and stands on the
original's ground line with its feet where the original's were. So units no
longer all fill the cell: a peasant is shorter than a dragon, and the four
frames do not change size. `--registration fill` restores the old
stretch-to-cell behaviour for comparison. This happens after matting.

Tune if needed (flags go **before** `batch`):

```sh
# poses lost only when IP-Adapter is on? loosen it (default 0.8)
python3 tools/gen_ai.py --controlnet --ipadapter --ip-weight 0.5 ... batch --category troops --only archers
# figure stiff or broken? ease the ControlNet off (default 1.0, all steps)
python3 tools/gen_ai.py --controlnet --ipadapter --cn-strength 0.7 ... batch --category troops --only archers
# character drifting between frames? tighten IP-Adapter (default 0.8)
python3 tools/gen_ai.py --controlnet --ipadapter --ip-weight 1.0 ... batch --category troops --only archers
# compare runs side by side (--out is global: it goes BEFORE batch/sheet)
python3 tools/gen_ai.py --out /tmp/run-a --controlnet --ipadapter ... batch --category troops --only archers
python3 tools/gen_ai.py --out /tmp/run-a sheet && open /tmp/run-a/index.html
```

`probe` lists the `weight_type` choices your IPAdapter node offers; `--ip-type`
must be one of them.

When archers look right, run the whole set with the same flags (resumable;
safe to stop):

```sh
PATH="$HOME/dev/ai/matte-venv/bin:$PATH" python3 tools/gen_ai.py --controlnet --ipadapter batch --category troops --resume 2>&1 | tee -a ~/ai-batch.log
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
| `ControlNet model ... not found` | § one-time ControlNet OpenPose above, then restart ComfyUI |
| `rembg not found` | run with `PATH="$HOME/dev/ai/matte-venv/bin:$PATH"` or `--rembg /full/path` |
| `No onnxruntime backend` | `pip install "rembg[cpu]"` in `~/dev/ai/matte-venv` |
| `recursive_mutex lock failed` after a frame | onnxruntime's macOS exit-time telemetry abort; `git pull` (script sets `ORT_DISABLE_TELEMETRY=1` and keeps the cut PNG) |
| `byte must be in range(0,256)` | old script; `git pull` to the latest version |
| black/garbled images | restart ComfyUI with `--force-fp16` |
| machine swapping hard | restart ComfyUI with `--lowvram` |

---

## Reporting back

Send: the `probe` version line, one generated unit's result (or how it looks),
and any traceback/`FAIL` lines. Don't commit — the primary Mac handles
promotion into `packs/open/art` and the built pack.
