#!/usr/bin/env python3
"""
Answer the King's Bounty manual-lookup copy protection.

The DOS release asks, at the character-select screen:

    "Please turn to page P, go to line L and type in word W."
    "Note: Spaces are not counted as words or lines. Hyphenated words are
     treated as one word. Any line that has any kind of text is considered
     a line."

This resolves that against the scanned manual you own
(`game-assets/kings-bounty_dos_manual_en.pdf`, which is gitignored -- this
script is a reader and ships no game content).

Scan geometry: each PDF sheet holds TWO printed pages side by side.

    PDF sheet = printed // 2 + 2      even printed = left half, odd = right

Verified against the printed folios: PDF sheet 10 carries printed 16/17, and
sheet 17 carries printed 30/31.

The PDF ships with an OCR text layer, but it is a poor read of the manual's
calligraphic face -- it renders "the" as "tfie", "his" as "fiis", "b" as "6"
and "g" as "9". This tool ignores that layer and re-OCRs the cropped half
page with tesseract, which is markedly more accurate. The crop is inset to
exclude the ornamental border, which otherwise contributes junk tokens at the
start and end of each line and corrupts word counting.

Requires: poppler (pdftoppm) and tesseract.

Usage:
    ./manual_word.py 17 12 7          # -> prisoners
    ./manual_word.py 17 12 7 --show   # also print the whole page, numbered
    ./manual_word.py --build          # pre-OCR every page into the cache
"""
import argparse
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.join(HERE, '..', 'game-assets')
PDF = os.path.join(ASSETS, 'kings-bounty_dos_manual_en.pdf')
# The cache holds OCR'd manual text, so it lives with the gitignored assets
# rather than beside this (tracked) script.
CACHE = os.path.join(ASSETS, '.manual_ocr')

DPI = 300
CROP = dict(y=250, w=1320, h=2010)
X_LEFT, X_RIGHT = 150, 1760

# Short tokens the ornamental border tends to leave at a line's edges.
DICT = set()
for _p in ('/usr/share/dict/words',):
    if os.path.exists(_p):
        DICT = {w.strip().lower() for w in open(_p, encoding='utf-8', errors='ignore')}
        break
REAL_SHORT = {'a', 'i', 'an', 'as', 'at', 'be', 'by', 'do', 'go', 'he', 'if',
              'in', 'is', 'it', 'me', 'my', 'no', 'of', 'on', 'or', 'so', 'to',
              'up', 'us', 'we', 'am', 'oh', 'ah', 'the', 'and', 'was', 'his',
              'her', 'had', 'has', 'for', 'you', 'all', 'out', 'who', 'but',
              'not', 'one', 'two', 'its', 'him', 'she', 'are', 'can', 'may'}


def sheet_for(printed: int):
    """Return (pdf_sheet, x_offset) for a printed page number."""
    return printed // 2 + 2, (X_RIGHT if printed % 2 else X_LEFT)


def is_junk(tok: str) -> bool:
    """True if a token looks like scanned border ornament, not a real word."""
    alpha = re.sub(r'[^A-Za-z]', '', tok)
    if not alpha:
        return True                       # pure punctuation, e.g. "}" or "|"
    if len(alpha) > 3:
        return False
    low = alpha.lower()
    if low in REAL_SHORT:
        return False
    if DICT and low in DICT and len(low) >= 3:
        return False
    return True


def clean(line: str) -> str:
    toks = line.split()
    while toks and is_junk(toks[0]):
        toks.pop(0)
    while toks and is_junk(toks[-1]):
        toks.pop()
    return ' '.join(toks)


def ocr_page(printed: int, force: bool = False) -> list[str]:
    """Return the cleaned, non-blank text lines of one printed page."""
    os.makedirs(CACHE, exist_ok=True)
    txt_path = os.path.join(CACHE, f'page_{printed:03d}.txt')
    if force or not os.path.exists(txt_path):
        sheet, x = sheet_for(printed)
        stem = os.path.join(CACHE, f'_r{printed}')
        subprocess.run(
            ['pdftoppm', '-r', str(DPI), '-f', str(sheet), '-l', str(sheet), '-png',
             '-x', str(x), '-y', str(CROP['y']), '-W', str(CROP['w']), '-H', str(CROP['h']),
             os.path.abspath(PDF), stem],
            check=True, capture_output=True)
        pngs = [p for p in os.listdir(CACHE) if p.startswith(f'_r{printed}') and p.endswith('.png')]
        if not pngs:
            raise SystemExit(f'render failed for printed page {printed}')
        img = os.path.join(CACHE, pngs[0])
        subprocess.run(['tesseract', img, txt_path[:-4], '--psm', '6'],
                       check=True, capture_output=True)
        os.remove(img)
    raw = open(txt_path, encoding='utf-8', errors='ignore').read().splitlines()
    out = []
    for ln in raw:
        c = clean(ln)
        if re.search(r'[A-Za-z]', c):     # "any line that has any kind of text"
            out.append(c)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('page', nargs='?', type=int)
    ap.add_argument('line', nargs='?', type=int)
    ap.add_argument('word', nargs='?', type=int)
    ap.add_argument('--show', action='store_true', help='print the page, line-numbered')
    ap.add_argument('--force', action='store_true', help='re-OCR, ignoring the cache')
    ap.add_argument('--build', action='store_true', help='pre-OCR pages 1..70')
    args = ap.parse_args()

    for tool in ('pdftoppm', 'tesseract'):
        if not shutil.which(tool):
            raise SystemExit(f'{tool} not found -- brew install poppler tesseract')

    if args.build:
        for p in range(1, 71):
            try:
                n = len(ocr_page(p, force=args.force))
                print(f'page {p:3}: {n} lines')
            except Exception as exc:                  # noqa: BLE001
                print(f'page {p:3}: {exc}')
        return 0

    if args.page is None:
        ap.error('give PAGE LINE WORD, or --build')

    lines = ocr_page(args.page, force=args.force)

    if args.show:
        sheet, x = sheet_for(args.page)
        side = 'right' if args.page % 2 else 'left'
        print(f'--- printed page {args.page} (PDF sheet {sheet}, {side} half) ---')
        for i, ln in enumerate(lines, 1):
            mark = ' <<<' if args.line and i == args.line else ''
            print(f'{i:3} | {ln}{mark}')
        print()

    if args.line is None or args.word is None:
        return 0

    if args.line > len(lines):
        raise SystemExit(f'page {args.page} has only {len(lines)} text lines')
    target = lines[args.line - 1]
    words = target.split()
    if args.word > len(words):
        raise SystemExit(f'line {args.line} has only {len(words)} words: {target!r}')

    answer = re.sub(r'^[^A-Za-z0-9]+|[^A-Za-z0-9]+$', '', words[args.word - 1])
    print(f'line {args.line}: {target}')
    print(f'word {args.word}: {answer}')
    print()
    print('Verify against the page image before typing it -- OCR of this '
          'calligraphic face is good but not perfect:')
    print(f'  ./manual_word.py {args.page} {args.line} {args.word} --show')
    return 0


if __name__ == '__main__':
    sys.exit(main())
