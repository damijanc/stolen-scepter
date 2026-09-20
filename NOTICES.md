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

## Open Art Pack

`packs/open/` holds original artwork created independently from a functional
spec, not derived from the original game's art. Its license is declared in
`packs/open/LICENSE`, and each file's authorship and method in
`packs/open/provenance.json`; contributors are listed in `packs/open/AUTHORS.md`.
`tools/open_pack.py verify` rejects any file that reproduces the original or an
upscale of it.
