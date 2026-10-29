#!/usr/bin/env python3
"""Generate the voxel models used for survival building block items.

Every `BuildingBlock` (common/src/comp/inventory/item/building.rs) gets two
original, procedurally generated MagicaVoxel models:

* `<name>.vox`      8x8x8 inventory icon, with patterned faces and baked face
                    shading so the cube reads well in the flat-lit item UI.
* `<name>-drop.vox` 5x5x5 model used when the item lies on the ground. It is
                    lit by the game, so no shading is baked in.

The block colours are parsed from `building.rs`, which is the single source of
truth, so the models always match the voxels that are placed in the world.

Usage:
    scripts/gen_building_blocks.py            # (re)write the .vox files
    scripts/gen_building_blocks.py --preview DIR
                                              # also render PNG previews that
                                              # mimic voxygen's item renderer

The models are deterministic: re-running the script yields identical files.
"""

import argparse
import math
import re
import struct
import subprocess
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
BUILDING_RS = ROOT / "common/src/comp/inventory/item/building.rs"
OUT_DIR = ROOT / "assets/voxygen/voxel/item/building_block"

ICON_SIZE = 8
DROP_SIZE = 5

# The first 22 palette indices have special meaning for figures (skin, hair,
# eyes, shiny, glowy, hollow, ...), see common/src/figure. Only use indices
# that are plain matte colours.
FIRST_PALETTE_INDEX = 33

# Item icon orientation: an isometric view that shows the three faces lit by
# the item renderer's model-space light, i.e. +x, +y and +z.
ICON_TILT_DEG = -math.degrees(math.atan(math.sqrt(2.0)))  # -54.74
ICON_YAW_DEG = -135.0
# Projected height of an isometric cube is sqrt(3) times its side length.
ICON_ZOOM = 0.55

MORTAR = (188, 180, 168)

# The item renderer lights every visible face with ~1.55x linear brightness,
# which blows out very light blocks. Darken their icons a little.
ICON_EXPOSURE = {"Snow": 0.80, "Ice": 0.92, "Sand": 0.94}


def snake_case(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def parse_blocks():
    """Return {VariantName: (r, g, b)} from building.rs."""
    src = BUILDING_RS.read_text()
    found = re.findall(
        r"BuildingBlock::(\w+) => \(BlockKind::\w+, \[(\d+), (\d+), (\d+)\]\)", src
    )
    if not found:
        raise SystemExit(f"no building block colours found in {BUILDING_RS}")
    return {name: tuple(int(c) for c in rgb) for name, *rgb in found}


def rnd(*args):
    """Deterministic hash noise in [0, 1)."""
    h = 0x811C9DC5
    for a in args:
        h ^= (a * 0x9E3779B1) & 0xFFFFFFFF
        h = (h * 0x01000193) & 0xFFFFFFFF
        h ^= h >> 15
    h = (h * 0x2C1B3C6D) & 0xFFFFFFFF
    h ^= h >> 12
    return (h & 0xFFFFFF) / float(0x1000000)


def jitter(amp, *args):
    """Noise multiplier quantised to 5 levels so palettes stay small."""
    level = int(rnd(*args) * 5)  # 0..4
    return 1.0 + amp * (level - 2) / 2.0


def scale(rgb, f):
    return tuple(max(0, min(255, round(c * f))) for c in rgb)


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def face_coords(x, y, z, n):
    """2D coordinates of a voxel on the face it is most visible from.

    Returns (face, h, v): face is 'top', 'bottom' or 'side'; on sides `h` runs
    horizontally around the cube and `v` is the height.
    """
    m = n - 1
    if z == m:
        return "top", x, y
    if z == 0:
        return "bottom", x, y
    if x == m or x == 0:
        return "side", y, z
    return "side", x, z


def bricks(h, v, row_height, brick_length, salt):
    """Return (is_mortar, brick_id) for a running bond brick pattern."""
    row = v // row_height
    if v % row_height == 0:
        return True, None
    shifted = h + (row % 2) * (brick_length // 2)
    if shifted % brick_length == 0:
        return True, None
    return False, (row, shifted // brick_length, salt)


def colour_at(block, colours, x, y, z, n):
    base = colours[block]
    face, h, v = face_coords(x, y, z, n)
    m = n - 1
    seed = (x, y, z, sum(base))

    if block == "Dirt":
        c = scale(base, jitter(0.16, *seed))
        if rnd(*seed, 1) < 0.10:
            c = scale(base, 0.74)
        elif rnd(*seed, 2) < 0.05:
            c = scale(base, 1.22)
        return c
    if block == "Turf":
        grass, dirt = base, colours["Dirt"]
        drip = (z == m - 1 and rnd(x, y, 7) < 0.55) or (
            z == m - 2 and rnd(x, y, 8) < 0.18
        )
        if z == m or drip:
            c = scale(grass, jitter(0.18, *seed))
            if z == m and rnd(*seed, 3) < 0.12:
                c = scale(grass, 1.2)
            return c
        return scale(dirt, jitter(0.16, *seed))
    if block == "Sand":
        c = scale(base, jitter(0.07, *seed))
        return scale(c, 1.07) if rnd(*seed, 1) < 0.12 else c
    if block == "Snow":
        c = scale(base, jitter(0.04, *seed))
        return mix(c, (190, 205, 230), 0.25) if rnd(*seed, 1) < 0.08 else c
    if block == "Stone":
        c = scale(base, jitter(0.10, *seed))
        crack = rnd(h // 2, v, face == "top", 4) < 0.12 and rnd(*seed, 5) < 0.7
        return scale(base, 0.74) if crack else c
    if block == "Timber":
        if face in ("top", "bottom"):
            heart = mix(base, colours["Planks"], 0.85)
            r = math.hypot(h - m / 2.0, v - m / 2.0)
            if r > m / 2.0 - 0.3:
                return scale(base, 0.9)
            return scale(heart, 0.86 if int(r * 1.3) % 2 else 1.0)
        groove = (h + int(rnd(h, v // 3, 6) * 2)) % 3 == 0
        return scale(base, 0.72 if groove else jitter(0.1, *seed))
    if block == "Leaves":
        r = rnd(*seed, 1)
        if r < 0.22:
            return scale(base, 0.68)
        if r < 0.30:
            return scale(base, 1.28)
        return scale(base, jitter(0.14, *seed))
    if block == "Ice":
        c = scale(base, jitter(0.05, *seed))
        if (h + v) % 5 == 0:
            return mix(c, (235, 248, 255), 0.45)
        return c
    if block == "Planks":
        if face in ("top", "bottom"):
            row, along = y, x
        else:
            row, along = v, h
        if row % 3 == 2:
            return scale(base, 0.68)
        plank = row // 3
        if along == (plank * 3 + 5) % n:
            return scale(base, 0.74)
        grain = jitter(0.06, plank, along // 2, row)
        return scale(base, grain * (0.94 if rnd(plank, row, 9) < 0.5 else 1.0))
    if block == "StoneBricks":
        if face in ("top", "bottom"):
            mortar, brick = bricks(x, y, 4, 4, 1)
        else:
            mortar, brick = bricks(h, v, 4, 4, 2)
        if mortar:
            return scale(base, 0.72)
        return scale(base, jitter(0.10, *brick) * jitter(0.04, *seed))
    if block == "ClayBricks":
        if face in ("top", "bottom"):
            mortar, brick = bricks(x, y, 3, 4, 1)
        else:
            mortar, brick = bricks(h, v, 3, 4, 2)
        if mortar:
            return scale(MORTAR, jitter(0.04, *seed))
        return scale(base, jitter(0.14, *brick) * jitter(0.05, *seed))
    raise SystemExit(f"no pattern for building block {block}")


def bake_shading(block, rgb, x, y, z, n):
    """Darken the side faces of icons so the cube has depth in the item UI,
    where every face visible from the icon angle receives the same light."""
    m = n - 1
    exposure = ICON_EXPOSURE.get(block, 1.0)
    if z == m:
        return scale(rgb, exposure)
    if x == m:
        return scale(rgb, 0.80 * exposure)
    if y == m:
        return scale(rgb, 0.92 * exposure)
    return rgb


def build_model(block, colours, n, baked):
    voxels = {}
    for x in range(n):
        for y in range(n):
            for z in range(n):
                c = colour_at(block, colours, x, y, z, n)
                if baked:
                    c = bake_shading(block, c, x, y, z, n)
                voxels[(x, y, z)] = c
    return voxels


def write_vox(path, n, voxels):
    palette = []
    index_of = {}
    for c in sorted(set(voxels.values())):
        index_of[c] = FIRST_PALETTE_INDEX + len(palette)
        palette.append(c)
    if FIRST_PALETTE_INDEX + len(palette) - 1 > 255:
        raise SystemExit(f"{path.name}: too many colours ({len(palette)})")

    def chunk(cid, content, children=b""):
        return cid + struct.pack("<ii", len(content), len(children)) + content + children

    size = chunk(b"SIZE", struct.pack("<iii", n, n, n))
    xyzi = struct.pack("<i", len(voxels)) + b"".join(
        struct.pack("<BBBB", x, y, z, index_of[c])
        for (x, y, z), c in sorted(voxels.items())
    )
    rgba = bytearray(256 * 4)
    for c, i in index_of.items():
        # RGBA entry k holds colour index k + 1.
        rgba[(i - 1) * 4 : i * 4] = bytes((*c, 255))
    data = b"VOX " + struct.pack("<i", 150)
    data += chunk(b"MAIN", b"", size + chunk(b"XYZI", xyzi) + chunk(b"RGBA", bytes(rgba)))
    path.write_bytes(data)


def rot_x(a):
    c, s = math.cos(a), math.sin(a)
    return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])


def rot_y(a):
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])


def rot_z(a):
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def icon_euler():
    """Express the isometric icon rotation as the (x, y, z) Euler angles used
    by `VoxTrans`, which builds `Rz(c) * Ry(b) * Rx(a)`."""
    r = rot_x(math.radians(ICON_TILT_DEG)) @ rot_z(math.radians(ICON_YAW_DEG))
    b = -math.asin(r[2, 0])
    a = math.atan2(r[2, 1], r[2, 2])
    c = math.atan2(r[1, 0], r[0, 0])
    rebuilt = rot_z(c) @ rot_y(b) @ rot_x(a)
    assert np.allclose(rebuilt, r), "Euler decomposition failed"
    return tuple(round(math.degrees(e), 2) for e in (a, b, c)), rebuilt


def srgb_to_linear(c):
    c = c / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(c):
    c = np.clip(c, 0.0, 1.0)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055) * 255.0


def render_preview(path, n, voxels, rotation, zoom, px=192):
    """Small CPU renderer mirroring voxygen's `draw_voxes`: orthographic,
    model-space light from (1, 1, 1), ambient 0.25, brightness 2.5 and the
    unoccluded AO factor of 3/4."""
    light = np.array([1.0, 1.0, 1.0]) / math.sqrt(3.0)
    img = np.zeros((px, px, 3))
    depth = np.full((px, px), -np.inf)
    s = 2.0 / n * zoom
    centre = np.array([n / 2.0] * 3)
    dirs = [np.array(d) for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))]
    ys, xs = np.mgrid[0:px, 0:px]
    ndc_x = (xs + 0.5) / px * 2 - 1
    ndc_y = 1 - (ys + 0.5) / px * 2
    for (x, y, z), c in voxels.items():
        for d in dirs:
            if tuple(np.array((x, y, z)) + d) in voxels:
                continue
            view_normal = rotation @ d
            if view_normal[2] <= 0:
                continue
            # Face corners in model space.
            axis = int(np.nonzero(d)[0][0])
            u_ax, v_ax = [a for a in range(3) if a != axis]
            o = np.array((x, y, z), dtype=float)
            if d[axis] > 0:
                o[axis] += 1
            corners = []
            for du, dv in ((0, 0), (1, 0), (1, 1), (0, 1)):
                p = o.copy()
                p[u_ax] += du
                p[v_ax] += dv
                corners.append(rotation @ ((p - centre) * s))
            corners = np.array(corners)
            # Rasterise the convex quad with edge functions.
            inside = np.ones((px, px), dtype=bool)
            area = 0.0
            for i in range(4):
                a, b = corners[i], corners[(i + 1) % 4]
                area += a[0] * b[1] - b[0] * a[1]
            sign = 1 if area > 0 else -1
            for i in range(4):
                a, b = corners[i], corners[(i + 1) % 4]
                e = (b[0] - a[0]) * (ndc_y - a[1]) - (b[1] - a[1]) * (ndc_x - a[0])
                inside &= e * sign >= 0
            z_face = corners[:, 2].mean()
            mask = inside & (z_face > depth)
            if not mask.any():
                continue
            lit = 0.75 * (max(0.0, float(d @ light)) + 0.25) * 2.5
            col = linear_to_srgb(srgb_to_linear(np.array(c, dtype=float)) * lit)
            img[mask] = col
            depth[mask] = z_face
    ppm = path.with_suffix(".ppm")
    with open(ppm, "wb") as f:
        f.write(b"P6 %d %d 255\n" % (px, px))
        f.write(img.astype(np.uint8).tobytes())
    subprocess.run(["magick", str(ppm), str(path)], check=True)
    ppm.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--preview", type=Path, help="write PNG previews here")
    args = parser.parse_args()

    colours = parse_blocks()
    euler, rotation = icon_euler()
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if args.preview:
        args.preview.mkdir(parents=True, exist_ok=True)

    for block in colours:
        name = snake_case(block)
        icon = build_model(block, colours, ICON_SIZE, baked=True)
        drop = build_model(block, colours, DROP_SIZE, baked=False)
        write_vox(OUT_DIR / f"{name}.vox", ICON_SIZE, icon)
        write_vox(OUT_DIR / f"{name}-drop.vox", DROP_SIZE, drop)
        if args.preview:
            render_preview(args.preview / f"{name}.png", ICON_SIZE, icon, rotation, ICON_ZOOM)
        print(f"{name}: {len(set(icon.values()))} icon colours, {len(set(drop.values()))} drop colours")

    print(
        "VoxTrans rotation: ({:.2f}, {:.2f}, {:.2f}), zoom {}".format(*euler, ICON_ZOOM)
    )


if __name__ == "__main__":
    main()
