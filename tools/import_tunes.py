"""Install the pack's effect tunes from Kenney's CC0 sound packs.

Walk, bump, chest and defeat are the four one-shots C plays
(`specs/audio-design.md`); they replace the square-wave beeps this pack used
to synthesise, so the effects match the orchestral music. The rest are the
port's own, which C does not have: the walk tune's desert, bridge, sailing
and flying variants, buying, finding something, and the combat sounds (hit,
shoot, slain, spell, victory).

The sources are four Kenney packs, all CC0 (https://kenney.nl): "Impact
Sounds", "RPG Audio", "Interface Sounds" and "Music Jingles". Download and unzip them anywhere,
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
# The defeat sting is a descending pizzicato phrase, which suits the strings
# in the combat track, and victory is a rising one. The combat sounds fire on
# every blow, so they sit a little under the overworld ones.
TUNES = {
    "audio/tune_bump.wav": ("impactSoft_medium_000.ogg", "Impact Sounds", None, 0.0, -6.0),
    "audio/tune_chest.wav": ("handleCoins.ogg", "RPG Audio", None, 0.1, -3.0),
    "audio/tune_defeat.wav": ("jingles_PIZZI14.ogg", "Music Jingles", None, 0.1, -3.0),
    "audio/tune_hit.wav": ("chop.ogg", "RPG Audio", None, 0.05, -8.0),
    "audio/tune_shoot.wav": ("drawKnife1.ogg", "RPG Audio", None, 0.08, -8.0),
    "audio/tune_slain.wav": ("impactPunch_heavy_000.ogg", "Impact Sounds", None, 0.1, -6.0),
    "audio/tune_spell.wav": ("glass_001.ogg", "Interface Sounds", None, 0.08, -6.0),
    "audio/tune_victory.wav": ("jingles_PIZZI10.ogg", "Music Jingles", None, 0.1, -3.0),
    "audio/tune_buy.wav": ("handleCoins2.ogg", "RPG Audio", None, 0.08, -6.0),
    "audio/tune_discover.wav": ("jingles_PIZZI15.ogg", "Music Jingles", None, 0.1, -3.0),
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

# Kenney has no water or wings, so these two are made here from filtered
# noise: an oar's swell for sailing and two wing beats for flight. Pack path
# -> ffmpeg lavfi source graph.
NOISE = {
    "audio/tune_sail.wav": (
        "anoisesrc=d=0.4:c=brown:a=0.9:r=44100:s=7,"
        "bandpass=f=500:width_type=h:w=700,"
        "afade=t=in:d=0.12:curve=qsin,afade=t=out:st=0.12:d=0.28:curve=qsin"
    ),
    "audio/tune_fly.wav": (
        "anoisesrc=d=0.36:c=pink:a=0.9:r=44100:s=11,lowpass=f=450,"
        "volume='if(lt(t,0.18),sin(PI*t/0.18),0.7*sin(PI*(t-0.18)/0.18))':eval=frame"
    ),
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


def hooves(first, second, pitch, out):
    """Two steps, 95 ms apart and pitched down, mixed into one hoof beat."""
    beat = (f"atrim=0:0.12,asetrate={RATE}*{pitch},aresample={RATE},"
            "aformat=channel_layouts=mono")
    run("ffmpeg", "-v", "error", "-y", "-i", first, "-i", second, "-filter_complex",
        f"[0]{beat}[a];[1]{beat},volume=0.8,adelay=95[b];"
        "[a][b]amix=inputs=2:normalize=0:duration=longest",
        "-c:a", "pcm_s16le", out)


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
    for rel, graph in NOISE.items():
        dst = os.path.join(PACK, rel)
        raw = dst + ".raw.wav"
        run("ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", graph, "-c:a", "pcm_s16le", raw)
        convert(raw, dst, None, 0.03, -12.0)
        os.remove(raw)
        register([rel], "procedural (tools/import_tunes.py)", method="procedural")
        print(rel)


if __name__ == "__main__":
    main()
