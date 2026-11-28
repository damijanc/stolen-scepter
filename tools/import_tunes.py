"""Install the pack's effect tunes from CC0 sound packs.

Walk, bump, chest and defeat are the four one-shots C plays
(`specs/audio-design.md`); they replace the square-wave beeps this pack used
to synthesise, so the effects match the orchestral music. The rest are the
port's own, which C does not have: the walk tune's desert, bridge, sailing
and flying variants, buying, finding something, and the combat sounds (hit,
shoot, slain, a sound per spell, victory).

The sources, all CC0:

- Four Kenney packs (https://kenney.nl): "Impact Sounds", "RPG Audio",
  "Interface Sounds" and "Music Jingles".
- OpenGameArt.org items, each named with its page in `RECORDINGS` below.

Download and unzip them anywhere, then point this at the folders holding
them; each source is found by file name anywhere under them:

    python3 tools/import_tunes.py ~/Downloads/kenney ~/Downloads/opengameart

Each file is cut, faded, peak-normalised and written as 16-bit mono WAV to
the path `game.json` names in `audio.tunes`, and provenance is recorded.
Needs ffmpeg.
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from packart import PACK, register  # noqa: E402

RATE = 44100

# Pack path -> (source file, source pack, seconds kept from the start (None
# keeps it all), fade-out seconds, peak dBFS).
#
# The defeat sting is a descending pizzicato phrase, which suits the strings
# in the combat track, and victory is a rising one. The combat sounds fire on
# every blow, so they sit a little under the overworld ones.
TUNES = {
    "audio/tune_bump.wav": ("impactSoft_medium_000.ogg", "Impact Sounds", None, 0.0, -6.0),
    "audio/tune_chest.wav": ("handleCoins.ogg", "RPG Audio", None, 0.1, -3.0),
    "audio/tune_defeat.wav": ("jingles_PIZZI14.ogg", "Music Jingles", None, 0.1, -3.0),
    "audio/tune_hit.wav": ("chop.ogg", "RPG Audio", None, 0.05, -8.0),
    "audio/tune_slain.wav": ("impactPunch_heavy_000.ogg", "Impact Sounds", None, 0.1, -6.0),
    "audio/tune_spell.wav": ("glass_001.ogg", "Interface Sounds", None, 0.08, -6.0),
    "audio/tune_victory.wav": ("jingles_PIZZI10.ogg", "Music Jingles", None, 0.1, -3.0),
    "audio/tune_buy.wav": ("handleCoins2.ogg", "RPG Audio", None, 0.08, -6.0),
    "audio/tune_discover.wav": ("jingles_PIZZI15.ogg", "Music Jingles", None, 0.1, -3.0),
    "audio/tune_tap.wav": ("tick_002.ogg", "Interface Sounds", None, 0.0, -14.0),
    "audio/tune_spell_turn_undead.wav": (
        "impactBell_heavy_000.ogg", "Impact Sounds", 1.5, 0.4, -6.0),
}

# The hero rides, so a step is a hoof's two-beat "clip-clop": two footsteps
# 95 ms apart, pitched down to sound heavier. Pack path -> (first step,
# second step, pitch factor). Grass for open ground, snow's crunch for sand,
# wood for a bridge's planks. Every step plays one, so they stay short and
# quiet (0.22 s, -12 dBFS).
HOOVES = {
    "audio/tune_walk.wav": ("footstep_grass_000.ogg", "footstep_grass_002.ogg", 0.75),
    "audio/tune_walk_desert.wav": ("footstep_snow_000.ogg", "footstep_snow_002.ogg", 0.8),
    "audio/tune_walk_bridge.wav": ("footstep_wood_000.ogg", "footstep_wood_002.ogg", 0.8),
}

# Recordings from OpenGameArt.org. Pack path -> (file, author, page, start
# second, seconds kept, fade-out seconds, peak dBFS). Several files hold one
# sound recorded at a few lengths, or a few wing beats; `start` picks one.
# The sail and fly sounds play on every step, so they are short and quiet;
# the splash is mostly quiet tail, so its peak sits higher to match the hooves.
OGA = "https://opengameart.org/content/"
RECORDINGS = {
    "audio/tune_shoot.wav": (
        "Bow.wav", "Ogrebane", "battle-sound-effects", 0.0, None, 0.08, -8.0),
    "audio/tune_spell_clone.wav": (
        "spell_01.ogg", "rubberduck", "80-cc0-rpg-sfx", 0.0, None, 0.1, -6.0),
    "audio/tune_spell_teleport.wav": (
        "teleport.wav", "Ogrebane", "teleport-spell", 0.0, 1.2, 0.3, -6.0),
    "audio/tune_spell_fireball.wav": (
        "105016__julien-matthey__jm-fx-fireball-01.wav", "Julien Matthey", "fireball-1",
        0.0, 1.2, 0.3, -6.0),
    "audio/tune_spell_lightning.wav": (
        "electricspell.ogg", "HaelDB", "spell-sounds", 16.29, 0.9, 0.15, -6.0),
    "audio/tune_spell_freeze.wav": (
        "ice.wav", "bart", "ice-spells", 0.0, 0.7, 0.1, -6.0),
    "audio/tune_spell_resurrect.wav": (
        "healing.ogg", "HaelDB", "spell-sounds", 11.5, 1.16, 0.2, -6.0),
    "audio/tune_sail.wav": (
        "splash_02.ogg", "rubberduck", "40-cc0-water-splash-slime-sfx", 0.0, 0.4, 0.15, -6.0),
    "audio/tune_fly.wav": (
        "wings_flap_large.ogg", "AntumDeluge", "large-wings-flap", 0.0, 0.5, 0.1, -12.0),
}


def find(roots, name):
    for root in roots:
        for dirpath, _, files in os.walk(root):
            if name in files:
                return os.path.join(dirpath, name)
    sys.exit(f"{name} not found under {', '.join(roots)}")


def run(*args):
    return subprocess.run(args, capture_output=True, text=True, check=True)


def convert(src, dst, keep, fade, peak, start=0.0):
    # First the take (from `start`), the leading silence off, mono, and the
    # cut, so the fade can be timed against what is left.
    cut = dst + ".cut.wav"
    chain = f"atrim=start={start},asetpts=PTS-STARTPTS," if start else ""
    chain += "silenceremove=start_periods=1:start_threshold=-50dB"
    if keep is not None:
        chain += f",atrim=0:{keep}"
    run("ffmpeg", "-v", "error", "-y", "-i", src, "-af", chain, "-ac", "1", "-ar", str(RATE),
        "-c:a", "pcm_s16le", cut)
    length = float(run("ffprobe", "-v", "error", "-show_entries", "format=duration",
                       "-of", "csv=p=0", cut).stdout)
    chain = f"afade=t=out:st={max(0.0, length - fade):.3f}:d={fade}" if fade else "anull"
    measured = run("ffmpeg", "-v", "info", "-i", cut, "-af", chain + ",volumedetect",
                   "-f", "null", "-").stderr
    gain = peak - float(re.search(r"max_volume: (-?[\d.]+) dB", measured).group(1))
    run("ffmpeg", "-v", "error", "-y", "-i", cut, "-af", f"{chain},volume={gain:.2f}dB",
        "-c:a", "pcm_s16le", dst)
    os.remove(cut)


def hooves(first, second, pitch, out):
    """Two steps, 95 ms apart and pitched down, mixed into one hoof beat."""
    beat = (f"atrim=0:0.12,asetrate={RATE}*{pitch},aresample={RATE},"
            "aformat=channel_layouts=mono")
    run("ffmpeg", "-v", "error", "-y", "-i", first, "-i", second, "-filter_complex",
        f"[0]{beat}[a];[1]{beat},volume=0.8,adelay=95[b];"
        "[a][b]amix=inputs=2:normalize=0:duration=longest",
        "-c:a", "pcm_s16le", out)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    root = sys.argv[1:]
    for rel, (name, pack, keep, fade, peak) in TUNES.items():
        convert(find(root, name), os.path.join(PACK, rel), keep, fade, peak)
        register(
            [rel],
            f'Kenney, "{pack}" ({name}), https://kenney.nl',
            method="downloaded, trimmed and normalised (tools/import_tunes.py)",
        )
        print(rel)
    for rel, (first, second, pitch) in HOOVES.items():
        dst = os.path.join(PACK, rel)
        mixed = dst + ".mix.wav"
        hooves(find(root, first), find(root, second), pitch, mixed)
        convert(mixed, dst, 0.22, 0.05, -12.0)
        os.remove(mixed)
        register(
            [rel],
            f'Kenney, "Impact Sounds" ({first}, {second}), https://kenney.nl',
            method="downloaded, mixed into a hoof beat (tools/import_tunes.py)",
        )
        print(rel)
    for rel, (name, author, page, start, keep, fade, peak) in RECORDINGS.items():
        convert(find(root, name), os.path.join(PACK, rel), keep, fade, peak, start)
        register(
            [rel],
            f"{author} ({name}), {OGA}{page}",
            method="downloaded, cut and normalised (tools/import_tunes.py)",
        )
        print(rel)

if __name__ == "__main__":
    main()
