# Open Art Pack

Original, independently authored art for the engine's rendered roles. This
directory is the only art that is committed: it is **not** an upscale, trace or
recolour of the original King's Bounty art, and must not become one.

## Clean-room rule

Art in `packs/open/art/` is created from the **functional spec only** -- path,
pixel size, frame count, alpha convention and how the engine composes it -- not
from the original pixels. Do not open, trace, sample or AI-upscale the original
art or anything under `game-assets/` while producing it. The spec is derived
from `tools/hd_assets.json`; `python3 tools/open_pack.py checklist` writes the
full slot list to `CHECKLIST.md`.

Keep character and place names original too. Do not ship the original game's
title, logo, or character names in a public pack.

## Adding art

1. Read `CHECKLIST.md` for the slots. Paths are pack-relative, so a listed
   `art/tiles/grass.png` lives at `packs/open/art/tiles/grass.png` here (drop
   the leading `art/`).
2. Draw at the native size (or a whole multiple for HD).
3. Add a `provenance.json` entry (see below).
4. Validate:

```sh
python3 tools/open_pack.py verify --scale 1     # native-size art
python3 tools/open_pack.py verify --scale 4     # 4x art
```

`verify` checks coverage, dimensions and alpha, that every present file has a
provenance entry, and that nothing is byte-identical to -- or a near match for
-- the original art or an upscale of it.

## Provenance schema

Each entry records who made the file and how, so a reviewer can audit it:

```json
{
  "assets": {
    "art/tiles/grass.png": {
      "author": "your name",
      "method": "hand-drawn",
      "source": "original",
      "license": "CC0-1.0",
      "date": "2026-01-01"
    }
  }
}
```

`source` must be `original` for anything under this pack (or a specific
permissive asset id). `method` might be `hand-drawn`, `procedural`,
`ai-assisted`, etc.

## Playtest

```sh
python3 tools/open_pack.py build --scale 1
cargo run -p openbounty-shell -- game-assets/open-pack/game.json
```

`build` overlays this pack's art onto the locally-cloned reference pack (which
stays gitignored) and writes the runnable result to `game-assets/open-pack`. A
partial pack still builds; the reference art shows through the slots that are
not covered yet.

## License

CC0 1.0 Universal (public domain dedication) -- see `LICENSE`. `provenance.json`
declares the pack license and every entry must agree with it; `verify` fails
otherwise.
