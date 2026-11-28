"""Install the pack's four effect tunes from Kenney's CC0 sound packs.

The tunes are walk, bump, chest and defeat -- the four one-shots the shell
plays (`specs/audio-design.md`). They replace the square-wave beeps this pack
used to synthesise, so the effects match the orchestral music.

The sources are three Kenney packs, all CC0 (https://kenney.nl): "Impact
Sounds", "RPG Audio" and "Music Jingles". Download and unzip them anywhere,
then point this at the folder holding them; each source is found by file name
anywhere under it:

    python3 tools/import_tunes.py ~/Downloads/kenney

Each file is trimmed, faded, peak-normalised and written as 16-bit mono WAV
to the path `game.json` names in `audio.tunes`, and provenance is recorded.
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
# The walk tick plays on every step, so it is short and quiet: the grass
# step's sound is all in its first 50 ms and the rest is room noise. The
# defeat sting is a descending pizzicato phrase, which suits the strings in
# the combat track.
TUNES = {
    "audio/tune_walk.wav": ("footstep_grass_004.ogg", "Impact Sounds", 0.15, 0.06, -12.0),
    "audio/tune_bump.wav": ("impactSoft_medium_000.ogg", "Impact Sounds", None, 0.0, -6.0),
    "audio/tune_chest.wav": ("handleCoins.ogg", "RPG Audio", None, 0.1, -3.0),
    "audio/tune_defeat.wav": ("jingles_PIZZI14.ogg", "Music Jingles", None, 0.1, -3.0),
}


def find(root, name):
    for dirpath, _, files in os.walk(root):
        if name in files:
            return os.path.join(dirpath, name)
    sys.exit(f"{name} not found under {root}")


def run(*args):
    return subprocess.run(args, capture_output=True, text=True, check=True)


def convert(src, dst, keep, fade, peak):
    # First the leading silence off, mono, and the cut, so the fade can be
    # timed against what is left.
    cut = dst + ".cut.wav"
    chain = "silenceremove=start_periods=1:start_threshold=-50dB"
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


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    root = sys.argv[1]
    for rel, (name, pack, keep, fade, peak) in TUNES.items():
        convert(find(root, name), os.path.join(PACK, rel), keep, fade, peak)
        register(
            [rel],
            f'Kenney, "{pack}" ({name}), https://kenney.nl',
            method="downloaded, trimmed and normalised (tools/import_tunes.py)",
        )
        print(rel)


if __name__ == "__main__":
    main()
