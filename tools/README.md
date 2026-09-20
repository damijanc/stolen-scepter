# Tools For The Original Game Materials

Readers for the local-only materials in `game-assets/`. These scripts are
tracked; the materials they read are not -- `game-assets/` and `reference/` are
gitignored per `AGENTS.md` ("never commit `reference/`, `game-assets/`, or
generated build output"). Nothing here reproduces game content.

On a fresh clone these tools will be present but the manual and game binaries
will not; supply your own copies under `game-assets/`.

The OCR cache these tools build is derived manual text, so it is written to
`game-assets/.manual_ocr/` (ignored) rather than next to the scripts.

## Prerequisites

```sh
brew install poppler tesseract          # required
brew install qpdf mupdf-tools ocrmypdf  # optional, generally useful for PDFs
```

`poppler` provides `pdftoppm` / `pdftotext` / `pdfimages`; `tesseract` does the
OCR. Nothing else is needed -- both tools are plain Python 3 with no pip
dependencies, and macOS ships a suitable `python3`.

## Running them

Both scripts resolve their own paths, so you can run them from anywhere:

```sh
# from the repo root
tools/manual_word.py 17 12 7

# or from this directory
cd tools && ./manual_word.py 17 12 7
```

If the executable bit is ever lost (e.g. after a copy), call them through the
interpreter instead:

```sh
python3 tools/manual_word.py 17 12 7
```

### First-time setup for `cc_extract.py`

The `.CC` archives ship inside the DOS zip and are NOT unpacked by default.
Extract them once (into a scratch directory -- do not add game binaries to the
repo):

```sh
mkdir -p /tmp/kb && unzip -o game-assets/Kings-Bounty_DOS_EN.zip -d /tmp/kb
ls /tmp/kb/kings-bounty        # 256.CC  416.CC  KB.EXE  KB!.COM
```

`manual_word.py` needs no setup beyond the manual PDF already being at
`game-assets/kings-bounty_dos_manual_en.pdf`.

## `manual_word.py` -- answer the manual-lookup copy protection

The DOS release asks at the character-select screen:

> Please turn to page P, go to line L and type in word W.
> Note: Spaces are not counted as words or lines. Hyphenated words are treated
> as one word. Any line that has any kind of text is considered a line.

```sh
./manual_word.py 17 12 7          # -> prisoners
./manual_word.py 17 12 7 --show   # print the whole page, line-numbered
./manual_word.py 17 --show        # just read a page, no lookup
./manual_word.py --build          # pre-OCR pages 1..70 into the cache
./manual_word.py 17 12 7 --force  # re-OCR this page, ignoring the cache
```

Typical session -- the game asks a different triple on each launch, so the
lookup matters more than any one answer:

```
$ ./manual_word.py 17 12 7
line 12: horrendously easy. Walk in, free the prisoners, and walk out.
word 7: prisoners
```

Type `prisoners`. The first call for a given page renders and OCRs it (a second
or two); afterwards it is cached in `game-assets/.manual_ocr/` and returns
instantly.

**Scan geometry.** Each PDF sheet holds TWO printed pages side by side:

```
PDF sheet = printed // 2 + 2     even printed = left half, odd = right half
```

Verified against the printed folios: sheet 10 carries printed 16/17, sheet 17
carries printed 30/31.

**Why it re-OCRs.** The PDF already ships an OCR text layer, but it is a poor
read of the manual's calligraphic face -- "the" comes out `tfie`, "his" as
`fiis`, `b` as `6`, `g` as `9`. That layer is still useful as an independent
cross-check on word POSITION (its word boundaries are mostly right), just not
on spelling. This tool ignores it and re-OCRs the cropped half-page with
tesseract 5.x, which is markedly better.

**Why it crops.** The ornamental border OCRs as junk tokens at the start and
end of each line (`Z}`, `aC`, `Q`, `Ss`, `Ge`), which would shift word counts.
The crop insets past the border; a dictionary-backed heuristic strips whatever
survives at the line edges.

**Always confirm with `--show`** before typing an answer. OCR of this face is
good but not perfect (it reads "turned" as `tumed`, "red" as `ted`). Position
is reliable; spelling is not. If in doubt, read the page image directly:

```sh
pdftoppm -r 300 -f 10 -l 10 -png \
  game-assets/kings-bounty_dos_manual_en.pdf /tmp/sheet
```

Note `ocrmypdf` refuses this file with `PriorOcrFoundError` because of that
existing layer. Use `--redo-ocr` (strips and redoes) or `--force-ocr`.

## `cc_extract.py` -- read the original `.CC` archives

Run it against the unpacked archives (see "First-time setup" above):

```sh
./cc_extract.py /tmp/kb/kings-bounty/256.CC                 # list + validate
./cc_extract.py /tmp/kb/kings-bounty/416.CC --out /tmp/cc   # also dump blobs
```

Every row prints `ok` when the entry decodes to exactly its declared
uncompressed size, so a clean run is a full integrity check of the archive:

```
256.CC: 66 entries, 281551 bytes
  entry   0 tag=0x12fa size=   6578         ok  alpha-runs=213
  ...
```

`alpha-runs` counts 4+ letter sequences in the decoded bytes. It is a noise
metric, not text -- see the findings below.

Format (independently derived here, and matching OpenBounty's
`reference/tools/extract_lzw.c`):

```
u16 LE  entry count
count x 8-byte rows:  u16 name hash | u24 offset | u24 compressed size
each blob:            u32 uncompressed size, then LZW
LZW:                  9..12 bit codes, LSB-first, 0x100 reset, 0x101 end
```

Entries are contiguous and the last ends exactly at EOF, so the directory is
self-validating. 256.CC has 66 entries, 416.CC has 133; all decode to exactly
their declared sizes.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `pdftoppm not found` / `tesseract not found` | `brew install poppler tesseract` |
| `No such file ... 256.CC` | The archives are still inside the zip -- see "First-time setup" |
| `page N has only M text lines` | OCR merged or dropped a line. Re-run with `--force`, then `--show` to inspect; fall back to reading the page image |
| The word looks misspelled | Expected on this face (`turned`->`tumed`, `red`->`ted`). Positions are reliable, spellings are not -- confirm with `--show` or the page image |
| The game rejects a correct-looking word | Check you counted the right page half. Even printed pages are the LEFT half of a sheet, odd are the RIGHT |
| `ocrmypdf` fails with `PriorOcrFoundError` | The scan already has a (poor) OCR layer. Use `--redo-ocr`, or `--force-ocr` to rebuild unconditionally |

To read a sheet directly, bypassing OCR entirely:

```sh
pdftoppm -r 300 -f 10 -l 10 -png \
  game-assets/kings-bounty_dos_manual_en.pdf /tmp/sheet   # -> /tmp/sheet-10.png
```

That renders printed pages 16 and 17 side by side, and is the ground truth
whenever the OCR is in doubt.

## Findings worth not rediscovering

- **Both `.CC` archives are pure graphics.** No English text in either --
  apparent "strings" are pixel runs.
- **`KB.EXE` is packed.** No game text is plaintext, not even "Select Char"
  which is visibly on screen. Ruled out: all 255 single-byte XOR keys, the
  bit-7-terminator encoding common to DOS-era games, and every standard packer
  signature (LZEXE, PKLITE, EXEPACK, DIET, aPACK, WWPACK). The longest
  printable run in the whole 79KB binary is 14 bytes of noise. The MZ header
  declares a ~1689-byte image with ~78KB trailing, i.e. a small loader stub
  plus appended packed data. Reading the protection's word table out of it
  would mean unpacking the stub first -- and it may hash rather than store the
  expected word. The manual lookup above is the cheaper route by far.
- **The web-transcription manual cannot answer the protection.** Its reflowed
  layout gives printed page 17 only 11 lines, and the check wants line 12. Its
  `--- N ---` markers are original page footers, so its page BOUNDARIES are
  right even though its line breaks are not.
