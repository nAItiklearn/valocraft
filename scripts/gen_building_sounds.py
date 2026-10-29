#!/usr/bin/env python3
"""Build the survival building place sounds from CC0 sources.

Sources: Kenney "Impact Sounds" 1.0 (https://kenney.nl/assets/impact-sounds),
CC0 1.0 (https://creativecommons.org/publicdomain/zero/1.0/). The individual
files are fetched from the CC0 Sounds mirror, which redistributes the pack
unmodified.

Each output layers two recordings so the sound reads on small speakers too:
a low impact for weight and a textured one for the material. The results are
mono, peak and loudness matched, trimmed, and encoded as Ogg Vorbis.

Usage: scripts/gen_building_sounds.py [--cache DIR]
Needs ffmpeg (with libvorbis) and numpy.
"""

import argparse
import subprocess
import urllib.request
from pathlib import Path

import numpy as np

MIRROR = "https://cc0-sounds.exi.software/sounds/kenney_impactsounds/Audio/"
OUT = Path(__file__).resolve().parent.parent / "assets/voxygen/audio/sfx/building"
RATE = 44100

# family -> [(layers: [(source, gain)])], one entry per variant
FAMILIES = {
    # Dirt, turf, sand, snow, leaves: a dull thud with a crumbly crunch.
    "soft": [
        [(f"impactSoft_medium_00{i}", 1.0), (f"footstep_snow_00{i}", 0.45)]
        for i in range(4)
    ],
    # Timber, planks: a wooden knock with a bit of plank rattle.
    "wood": [
        [(f"impactWood_medium_00{i}", 1.0), (f"impactPlank_medium_00{i}", 0.3)]
        for i in range(4)
    ],
    # Stone, bricks, ice: a gritty clack with some weight underneath.
    "stone": [
        [(f"footstep_concrete_00{c}", 0.9), (f"impactSoft_medium_00{s}", 0.55)]
        for c, s in [(0, 1), (3, 2), (4, 3), (1, 4)]
    ],
}

PEAK_DB = -1.5
RMS_DB = -17.0
TAIL_DB = -45.0
FADE_S = 0.02


def fetch(name: str, cache: Path) -> Path:
    path = cache / f"{name}.ogg"
    if not path.exists():
        cache.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(MIRROR + path.name, timeout=30) as r:
            path.write_bytes(r.read())
    return path


def decode(path: Path) -> np.ndarray:
    raw = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(path), "-ac", "1", "-ar", str(RATE),
         "-f", "f32le", "-"],
        check=True, capture_output=True,
    ).stdout
    return np.frombuffer(raw, dtype=np.float32).astype(np.float64)


def envelope(x: np.ndarray) -> np.ndarray:
    k = int(0.005 * RATE)
    return np.convolve(np.abs(x), np.ones(k) / k, "same")


def onset(x: np.ndarray) -> int:
    env = envelope(x)
    return int(np.argmax(env > env.max() * 10 ** (-30 / 20)))


def mix(layers) -> np.ndarray:
    # Align the onsets so the layers hit together.
    parts = [(x[onset(x):], gain) for x, gain in layers]
    out = np.zeros(max(len(x) for x, _ in parts))
    for x, gain in parts:
        out[: len(x)] += x / np.abs(x).max() * gain
    # Trim the silent tail and fade out.
    env = envelope(out)
    end = len(out) - int(np.argmax(env[::-1] > env.max() * 10 ** (TAIL_DB / 20)))
    out = out[: min(len(out), end + int(FADE_S * RATE))]
    fade = min(len(out), int(FADE_S * RATE))
    out[-fade:] *= np.linspace(1.0, 0.0, fade)
    # Match loudness, then make sure the peak stays below PEAK_DB.
    audible = out[np.abs(out) > np.abs(out).max() * 10 ** (-40 / 20)]
    out *= 10 ** (RMS_DB / 20) / np.sqrt(np.mean(audible**2))
    peak = np.abs(out).max()
    if peak > 10 ** (PEAK_DB / 20):
        out *= 10 ** (PEAK_DB / 20) / peak
    return out


def encode(x: np.ndarray, path: Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-f", "f32le", "-ar", str(RATE), "-ac", "1",
         "-i", "-", "-c:a", "libvorbis", "-q:a", "5", "-map_metadata", "-1",
         str(path)],
        input=x.astype(np.float32).tobytes(), check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cache", type=Path, default=Path("/tmp/opencode/kenney/src"))
    args = parser.parse_args()
    for family, variants in FAMILIES.items():
        for i, layers in enumerate(variants, start=1):
            x = mix([(decode(fetch(name, args.cache)), gain) for name, gain in layers])
            path = OUT / f"place_{family}_{i}.ogg"
            encode(x, path)
            print(f"{path.relative_to(OUT.parent.parent.parent.parent.parent)}: "
                  f"{len(x) / RATE * 1000:.0f} ms, peak {20 * np.log10(np.abs(x).max()):.1f} dB")


if __name__ == "__main__":
    main()
