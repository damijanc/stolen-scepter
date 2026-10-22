# Notices

This project is a faithful Rust port of the engine behavior from
OpenBounty, an MIT-licensed C/raylib reimplementation of King's Bounty.

Reference project:

- OpenBounty: <https://github.com/dannyheskett/openbounty>
- License: MIT
- Copyright: retained by the OpenBounty contributors.

Mechanically ported engine logic in this repository should preserve the
OpenBounty copyright and permission notice in distributions that include it.

No original King's Bounty assets are shipped here. Game data and media should
remain bring-your-own and be extracted from a legally owned copy using the
OpenBounty asset extractor workflow.

## Independent Content

The goal is a pack that is entirely our own work (art, maps, text, audio), so
the game can be distributed without any file from the original. That pack is
being built; see `specs/publish-plan.md`. The art generators in `tools/` take
only functional geometry from the original (sizes, ground lines, frame
counts, as numbers in `tools/asset_layout.json`) and never its pixels.
