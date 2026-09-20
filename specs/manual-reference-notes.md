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

## Local Tooling (`tools/`, see `tools/README.md`)

Readers for the local materials live in `tools/`. The scripts are tracked; the
materials they read are not, so on a fresh clone the tools will be present but
`game-assets/` will be empty. Their OCR cache is derived manual text and is
written to `game-assets/.manual_ocr/`, which the existing ignore rule covers.

- `manual_word.py` answers the DOS manual-lookup copy protection ("page P,
  line L, word W") against the scanned manual. Each PDF sheet holds two printed
  pages, so `PDF sheet = printed // 2 + 2`, even printed page on the left half.
- `cc_extract.py` reads the original `256.CC` / `416.CC` archives: `u16` entry
  count, 8-byte directory rows (`u16` name hash, `u24` offset, `u24` size), each
  blob a `u32` uncompressed size followed by 9..12-bit LSB-first LZW. Same
  format OpenBounty's `reference/tools/extract_lzw.c` implements.

Two findings that bound how much the local binaries can ever tell us:

- Both `.CC` archives are PURE GRAPHICS. Neither holds any English text.
- `KB.EXE` is packed. No game text is plaintext -- not even "Select Char",
  which is visibly on screen. Single-byte XOR, bit-7-terminator encoding and
  every standard packer signature were ruled out. Extracting anything textual
  from it requires unpacking the loader stub first.

The scanned manual (`kings-bounty_dos_manual_en.pdf`) carries an OCR text
layer, but it is a poor read of the calligraphic face -- "the" as `tfie`,
"his" as `fiis`, `b` as `6`, `g` as `9`. Treat its word POSITIONS as usable
and its spellings as not; re-OCR with tesseract 5.x for anything quotable.
