"""Where the art generators write, and the provenance record they share.

Every generator writes into the game's own pack, `packs/stolen-scepter/`, and
records each file it wrote in that pack's `provenance.json` (author, method,
licence), so the credits and the CI gate can account for every image.
"""
import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACK = os.path.join(ROOT, "packs", "stolen-scepter")
ART = os.path.join(PACK, "art")
PROVENANCE = os.path.join(PACK, "provenance.json")


def out_dir(*parts):
    """`art/<parts>` inside the pack, created on first use."""
    path = os.path.join(ART, *parts)
    os.makedirs(path, exist_ok=True)
    return path


def register(rels, author, method="procedural", license="CC0-1.0"):
    """Record `rels` (pack-relative paths) as written by `author`."""
    if os.path.exists(PROVENANCE):
        with open(PROVENANCE) as fh:
            doc = json.load(fh)
    else:
        doc = {"assets": {}}
    for rel in rels:
        doc["assets"][rel] = {
            "author": author,
            "method": method,
            "license": license,
        }
    doc["assets"] = dict(sorted(doc["assets"].items()))
    with open(PROVENANCE, "w") as fh:
        json.dump(doc, fh, indent=2)
        fh.write("\n")
