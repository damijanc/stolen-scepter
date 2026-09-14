# Manual And Binary Reference Notes

## Policy

- Keep `game-assets/` local-only. The manual and platform binaries are reference
  inputs, not source files for the Rust port, and should not be committed.
- OpenBounty remains the parity target. Use the manual as secondary evidence for
  data sanity checks, labels, and user-facing game facts.
- The supplied PDF is named as a DOS manual, but its setup pages describe Sega
  Mega Drive controls. Treat edition-specific instructions with caution.

## Local Reference Assets

- `game-assets/Kings-Bounty_Manual_DOS_EN_Web-Manual.pdf`
  - 28 pages.
  - Useful appendices: castle/town locations, troop chart, morale chart,
    character advancement chart, villain-to-castle worksheet, spell-to-town
    worksheet.
- `game-assets/Kings-Bounty_DOS_EN.zip`
  - Contains `KB.EXE`, `KB!.COM`, `256.CC`, and `416.CC`.
- `game-assets/Kings-Bounty_Apple-II_EN.zip`
- `game-assets/Kings-Bounty_Amiga_EN.zip`
- `game-assets/Kings-Bounty_C64_EN.zip`

## High-Value Manual Facts To Cross-Check

- Starting classes:
  - Knight: leadership 100, max spells 2, spell power 1, weekly commission 1000.
  - Paladin: leadership 80, max spells 3, spell power 1, weekly commission 1000.
  - Sorceress: leadership 60, max spells 5, spell power 2, weekly commission 3000.
  - Barbarian: leadership 100, max spells 2, spell power 0, weekly commission 2000.
- Archmage Aurange teaches magic in Continentia at `(11, 19)` for 5000 gold.
- Siege weapons cost 3000 gold.
- Standard boat rental is 500 gold; the Anchor of Admiralty lowers it to 100.
- The puzzle map has 25 pieces: 16 with villains, 1 with Arech, and 8 hidden
  with artifacts.
- The manual troop chart can sanity-check troop stats, morale groups, and
  dwelling categories.
- The morale chart matches the current default matrix:
  `NNNNN`, `NNNNN`, `NNHNN`, `LNLHN`, `LLLNN`.

## Candidate Follow-Up Fixtures

- Compare OpenBounty `game.json` troop stats against the manual troop chart.
- Compare class/rank advancement against Appendix C.
- Compare resource town/castle coordinates against Appendix A.
- Compare spell town pinning/availability against Appendix E only as a
  reference worksheet; OpenBounty output remains authoritative for randomized
  spell salting.
- Use DOS `KB.EXE` only through extraction tooling or offline inspection. Do not
  make the engine crate depend on local binaries.
