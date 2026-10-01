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

The near-match check that rejects traced or upscaled art has no audio
equivalent. Nothing automated can tell an original composition from a rip of
the original soundtrack, so every audio file's licence rests on human review
at the point of sourcing. The clean-room rule covers melodies: re-rendering
the original game's PC-speaker tunes with a better synth produces a
derivative of the original composition, not original work.

## Adding audio

The six audio slots (two OGG Vorbis music loops, four WAV one-shot tunes) are
listed at the end of `CHECKLIST.md` with their sample rate, duration and size
bounds. They live under `packs/open/audio/`, at the same pack-relative path the
engine's `audio` block names, and need a `provenance.json` entry like any art.
Third-party CC0 audio is fine: give its asset id as `source`. `verify` checks
format, sample rate, channels, duration, size and provenance; `build` overlays
the audio onto the playtest pack alongside the art.

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
cargo run -p openbounty-shell -- --pack game-assets/open-pack
```

`build` overlays this pack's art onto the locally-cloned reference pack (which
stays gitignored) and writes the runnable result to `game-assets/open-pack`. A
partial pack still builds; the reference art shows through the slots that are
not covered yet.

## License

CC0 1.0 Universal (public domain dedication) -- see `LICENSE`. `provenance.json`
declares the pack license and every entry must agree with it; `verify` fails
otherwise.
