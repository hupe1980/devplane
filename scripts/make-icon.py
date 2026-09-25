#!/usr/bin/env python3
"""Writes the app's icons: icons/icon.png (512), icons/128x128.png, icons/32x32.png.

Pure python — `zlib` and arithmetic, no PIL — so the icons are regenerated
by one command on any machine and never drawn by hand in a tool nobody else
has. The PNGs are committed; this is how they are remade.

Deterministic: the same script writes the same bytes, so a diff in `icons/`
is a change to this file and nothing else.

    python3 scripts/make-icon.py
"""

import struct
import zlib
from pathlib import Path

# The product's two colours, from ui/src/tokens.css: `--ink` and `--bg`.
INK = (0x1C, 0x1B, 0x19)
BG = (0xFB, 0xFB, 0xFA)

# Supersampling per axis; 4×4 samples per pixel is enough for a 32px icon.
SS = 4


def coverage(size, x, y):
    """How much of pixel (x, y) is ink, in [0, 1], for an icon of `size`."""
    n = size
    hit = 0
    for sy in range(SS):
        for sx in range(SS):
            px = (x + (sx + 0.5) / SS) / n
            py = (y + (sy + 0.5) / SS) / n
            if inside(px, py):
                hit += 1
    return hit / (SS * SS)


def rounded_square(px, py, inset, radius):
    """A square with rounded corners, in unit coordinates."""
    lo, hi = inset, 1 - inset
    if px < lo or px > hi or py < lo or py > hi:
        return False
    cx = min(max(px, lo + radius), hi - radius)
    cy = min(max(py, lo + radius), hi - radius)
    return (px - cx) ** 2 + (py - cy) ** 2 <= radius**2


def glyph(px, py):
    """A bold D: a stem and a half-disc, the shape of the wordmark's first letter."""
    stem = 0.30 <= px <= 0.42 and 0.26 <= py <= 0.74
    if stem:
        return True
    # The bowl: an outer half-disc minus an inner one, right of the stem.
    if px < 0.36:
        return False
    cx, cy = 0.40, 0.50
    r_out, r_in = 0.24, 0.12
    d2 = (px - cx) ** 2 + (py - cy) ** 2
    return r_in**2 <= d2 <= r_out**2


def inside(px, py):
    """Ink where the plate is and the glyph is not; the glyph is the plate's colour."""
    return rounded_square(px, py, 0.06, 0.20) and not glyph(px, py)


def plate(px, py):
    return rounded_square(px, py, 0.06, 0.20)


def render(size):
    rows = []
    for y in range(size):
        row = bytearray([0])  # filter type 0
        for x in range(size):
            # Alpha is the plate; colour blends ink over the light glyph.
            a = 0
            ink = 0
            for sy in range(SS):
                for sx in range(SS):
                    px = (x + (sx + 0.5) / SS) / size
                    py = (y + (sy + 0.5) / SS) / size
                    if plate(px, py):
                        a += 1
                        if not glyph(px, py):
                            ink += 1
            if a == 0:
                row += bytes((0, 0, 0, 0))
                continue
            t = ink / a
            r = round(BG[0] + (INK[0] - BG[0]) * t)
            g = round(BG[1] + (INK[1] - BG[1]) * t)
            b = round(BG[2] + (INK[2] - BG[2]) * t)
            row += bytes((r, g, b, round(255 * a / (SS * SS))))
        rows.append(bytes(row))
    return b"".join(rows)


def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


def png(size, raw):
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def main():
    out = Path(__file__).resolve().parent.parent / "icons"
    out.mkdir(exist_ok=True)
    for size, name in ((512, "icon.png"), (128, "128x128.png"), (32, "32x32.png")):
        path = out / name
        path.write_bytes(png(size, render(size)))
        print(f"make-icon: {path.relative_to(out.parent)} ({size}×{size})")


if __name__ == "__main__":
    main()
