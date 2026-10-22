"""Synthesise the pack's four effect tunes: walk, bump, chest and defeat.

Square and triangle voices with short envelopes, in the spirit of a PC-speaker
game but written from scratch here, so the tunes carry no third-party licence.
Lengths follow how the shell uses them (`specs/audio-design.md`): the walk tick
plays on every step and must be tiny; the chest and defeat jingles duck the
music for about two seconds.

Writes 16-bit mono WAVs to the paths `game.json` names in `audio.tunes` and
registers provenance.
"""
import json
import math
import os
import struct
import sys
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from packart import PACK, ROOT, register  # noqa: E402

RATE = 22050
AUTHOR = "procedural (tools/gen_tunes.py)"


def hz(note):
    """MIDI note number to frequency."""
    return 440.0 * 2 ** ((note - 69) / 12)


def voice(freq, seconds, shape="square", volume=0.35, attack=0.004, release=0.05,
          slide=0.0):
    """One note: `slide` bends the pitch by that many semitones over its length."""
    n = int(RATE * seconds)
    out = []
    phase = 0.0
    for i in range(n):
        t = i / RATE
        f = freq * 2 ** (slide * (i / n) / 12)
        phase = (phase + f / RATE) % 1.0
        if shape == "square":
            s = 1.0 if phase < 0.5 else -1.0
        elif shape == "triangle":
            s = 4 * abs(phase - 0.5) - 1
        else:
            s = math.sin(2 * math.pi * phase)
        env = min(1.0, t / attack) if attack else 1.0
        tail = seconds - t
        if tail < release:
            env *= tail / release
        out.append(s * env * volume)
    return out


def rest(seconds):
    return [0.0] * int(RATE * seconds)


def mixdown(*tracks):
    n = max(len(t) for t in tracks)
    return [sum(t[i] for t in tracks if i < len(t)) for i in range(n)]


def melody(notes, step, shape, volume=0.3, gap=0.012):
    """`notes` as (midi or None, beats); each beat lasts `step` seconds."""
    out = []
    for note, beats in notes:
        length = beats * step
        if note is None:
            out += rest(length)
        else:
            out += voice(hz(note), length - gap, shape, volume, release=min(0.08, length / 3))
            out += rest(gap)
    return out


def walk():
    # A soft, very short tick: two cycles of a high triangle.
    return voice(1200, 0.018, "triangle", 0.25, attack=0.001, release=0.012)


def bump():
    # A low, buzzy blip that drops a few semitones: "can't go there".
    return voice(110, 0.09, "square", 0.3, attack=0.002, release=0.03, slide=-5)


def chest():
    # A bright rising arpeggio that lands on a held major chord.
    step = 0.09
    lead = melody([(72, 1), (76, 1), (79, 1), (84, 1), (79, 1), (84, 3), (88, 6)],
                  step, "square", 0.22)
    bass = melody([(None, 4), (48, 2), (55, 2), (60, 6)], step * 1.05, "triangle", 0.3)
    return mixdown(lead, bass)


def defeat():
    # A slow falling minor line over a sinking bass.
    step = 0.16
    lead = melody([(69, 2), (68, 2), (67, 2), (66, 2), (65, 5)], step, "square", 0.2)
    bass = melody([(45, 4), (44, 4), (41, 5)], step, "triangle", 0.32)
    return mixdown(lead, bass)


def write_wav(path, samples):
    peak = max(1e-9, max(abs(s) for s in samples))
    gain = min(1.0, 0.9 / peak)
    with wave.open(path, "wb") as fh:
        fh.setnchannels(1)
        fh.setsampwidth(2)
        fh.setframerate(RATE)
        fh.writeframes(b"".join(struct.pack("<h", int(s * gain * 32767)) for s in samples))


def main():
    with open(os.path.join(PACK, "game.json")) as fh:
        tunes = json.load(fh)["audio"]["tunes"]
    makers = {"walk": walk, "bump": bump, "chest": chest, "defeat": defeat}
    written = []
    for name, rel in tunes.items():
        path = os.path.join(PACK, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        samples = makers[name]()
        write_wav(path, samples)
        written.append(rel)
        print(f"{rel}: {len(samples) / RATE:.2f}s")
    register(written, AUTHOR)
    print(f"wrote {len(written)} tunes to {os.path.relpath(PACK, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
