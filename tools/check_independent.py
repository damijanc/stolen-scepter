"""Fail if the repository contains anything from the original game.

Three checks over every tracked file:

  paths   nothing is tracked under reference/ or game-assets/;
  files   no tracked file is byte-identical to an original file (SHA-256 of
          every original art, audio, map and palette file);
  text    no tracked text contains a proper name from the original
          (villains, artifacts, towns, castles, continents), or a run of six
          words copied from its longer texts (intro, bios, messages).

The reference data is stored as SHA-256 hashes only, in
`tools/independent_hashes.json`, so the check never publishes the names or
text it looks for. Names that are ordinary English words are left out, or
normal prose would trip the check.

    python3 tools/check_independent.py           # check (CI)
    python3 tools/check_independent.py --build   # rebuild the hashes from a
                                                 # locally owned copy

`--build` needs `reference/assets/kings-bounty/` (and, if present, the
extracted art under `game-assets/extracted/`); the check needs neither.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HASHES = os.path.join(ROOT, "tools", "independent_hashes.json")
PACK = os.path.join(ROOT, "reference", "assets", "kings-bounty")
EXTRACTED = os.path.join(ROOT, "game-assets", "extracted")
DICT = "/usr/share/dict/words"

FORBIDDEN_DIRS = ("reference/", "game-assets/")
# TEMPORARY, until Phase 2 of specs/publish-plan.md: the art slot list still
# follows the original pack's file naming (villain and continent ids in the
# slot paths). Phase 2 regenerates it from the new pack; remove these then.
# Only the TEXT check is skipped for them -- the path and file checks still run.
TEXT_EXEMPT = ()
NAME_SOURCES = ("villains", "artifacts", "towns", "castles", "zones")
SHINGLE = 6          # words per copied-text window
MAX_NGRAM = 5        # longest name, in words


def sha(data):
    return hashlib.sha256(data).hexdigest()


def words(text):
    """Lower-case word tokens; apostrophes kept inside words."""
    text = text.lower().replace("’", "'")
    return re.findall(r"[a-z0-9]+(?:'[a-z0-9]+)*", text)


def ngrams(tokens, n_max):
    for n in range(1, n_max + 1):
        for i in range(len(tokens) - n + 1):
            yield " ".join(tokens[i:i + n])


def strings_of(node):
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        for v in node.values():
            yield from strings_of(v)
    elif isinstance(node, list):
        for v in node:
            yield from strings_of(v)


def build():
    if not os.path.isdir(PACK):
        sys.exit(f"--build needs the local original pack at {os.path.relpath(PACK, ROOT)}")
    common = set()
    if os.path.exists(DICT):
        with open(DICT) as fh:
            common = {w.strip().lower() for w in fh if w.strip()}
    with open(os.path.join(PACK, "game.json")) as fh:
        data = json.load(fh)

    names = set()
    for key in NAME_SOURCES:
        for rec in data.get(key, []):
            name = rec.get("name") if isinstance(rec, dict) else None
            toks = words(name or "")
            if toks and toks[0] == "the":
                toks = toks[1:]
            if not toks or len(toks) > MAX_NGRAM:
                continue
            # A one-word name that is an ordinary word ("Anomaly") would flag
            # normal prose; names of two or more words are distinctive.
            if len(toks) == 1 and toks[0] in common:
                continue
            names.add(" ".join(toks))

    shingles = set()
    for text in strings_of(data.get("strings", {})):
        toks = words(text)
        for i in range(len(toks) - SHINGLE + 1):
            shingles.add(" ".join(toks[i:i + SHINGLE]))

    files = set()
    for base in (PACK, os.path.join(EXTRACTED, "art")):
        for dirpath, _, fnames in os.walk(base):
            for f in fnames:
                if f == "game.json" or f.startswith("."):
                    continue
                with open(os.path.join(dirpath, f), "rb") as fh:
                    files.add(sha(fh.read()))

    doc = {
        "schema": 1,
        "generated_by": "tools/check_independent.py --build",
        "description": ("SHA-256 hashes of the original game's proper names, six-word text "
                        "windows and media files. Hashes only, so nothing original is "
                        "published here."),
        "names": sorted(sha(n.encode()) for n in names),
        "shingles": sorted(sha(s.encode()) for s in shingles),
        "files": sorted(files),
    }
    with open(HASHES, "w") as fh:
        json.dump(doc, fh, indent=0)
        fh.write("\n")
    print(f"{os.path.relpath(HASHES, ROOT)}: {len(names)} names, {len(shingles)} text "
          f"windows, {len(files)} files")
    return 0


def tracked():
    out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, check=True,
                         capture_output=True).stdout
    return [p for p in out.decode().split("\0") if p]


def check(show=False):
    with open(HASHES) as fh:
        ref = json.load(fh)
    names, shingles, files = set(ref["names"]), set(ref["shingles"]), set(ref["files"])
    me = os.path.relpath(HASHES, ROOT)
    problems = []
    for path in tracked():
        if path.startswith(FORBIDDEN_DIRS):
            problems.append(f"{path}: tracked under a local-only directory")
            continue
        full = os.path.join(ROOT, path)
        if not os.path.isfile(full) or path == me:
            continue
        with open(full, "rb") as fh:
            data = fh.read()
        if sha(data) in files:
            problems.append(f"{path}: identical to an original game file")
            continue
        if b"\0" in data[:8192] or path in TEXT_EXEMPT:
            continue  # binary (already hash-checked), or exempt until Phase 2
        for lineno, line in enumerate(data.decode("utf-8", "replace").splitlines(), 1):
            toks = words(line)
            hit = next((g for g in ngrams(toks, MAX_NGRAM)
                        if sha(g.encode()) in names), None)
            if hit is None:
                hit = next((" ".join(toks[i:i + SHINGLE])
                            for i in range(len(toks) - SHINGLE + 1)
                            if sha(" ".join(toks[i:i + SHINGLE]).encode()) in shingles), None)
            if hit is not None:
                # The match is printed only with --show (local use): CI logs of
                # a public repository are public too.
                what = f" [{hit}]" if show else ""
                problems.append(f"{path}:{lineno}: text from the original game{what}")
    for p in problems:
        print(p)
    print(f"check_independent: {len(problems)} problem(s) in {len(tracked())} tracked files")
    return 1 if problems else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--build", action="store_true",
                    help="rebuild the hash lists from a locally owned copy")
    ap.add_argument("--show", action="store_true",
                    help="print what matched (local use only; never in CI)")
    args = ap.parse_args()
    return build() if args.build else check(args.show)


if __name__ == "__main__":
    sys.exit(main())
