#!/usr/bin/env python3
"""Draws Spindle's icon: a green mirror ball, hung upside down (the hook at
the bottom), with a sparkle. Writes the master PNGs the packaging uses:

    packaging/icons/spindle-1024.png      transparent background (Windows, tray)
    packaging/icons/spindle-mac-1024.png  dark rounded square (macOS)

Pure Python with Pillow, so it runs anywhere: python3 packaging/icons/render-icon.py
"""

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

HERE = Path(__file__).resolve().parent
SIZE = 1024
SUPER = 2  # supersampling
S = SIZE * SUPER


def shade(nx, ny, nz):
    """Lambert plus a specular highlight, light from the upper left."""
    lx, ly, lz = -0.45, -0.55, 0.7
    norm = math.sqrt(lx * lx + ly * ly + lz * lz)
    lx, ly, lz = lx / norm, ly / norm, lz / norm
    diffuse = max(0.0, nx * lx + ny * ly + nz * lz)
    # reflection vector for the specular term, viewer along +z
    dot = nx * lx + ny * ly + nz * lz
    rz = 2 * dot * nz - lz
    specular = max(0.0, rz) ** 28
    return diffuse, specular


def mirror_ball(size):
    """A sphere of mirror tiles on a transparent canvas of `size` pixels."""
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    cx = cy = size / 2
    radius = size * 0.40
    rows = 22
    # Base sphere, dark green, under the tiles.
    draw.ellipse(
        (cx - radius, cy - radius, cx + radius, cy + radius),
        fill=(6, 44, 22, 255),
    )
    for row in range(rows):
        lat0 = -math.pi / 2 + math.pi * row / rows
        lat1 = -math.pi / 2 + math.pi * (row + 1) / rows
        mid_lat = (lat0 + lat1) / 2
        columns = max(6, int(round(44 * math.cos(mid_lat))))
        for column in range(columns):
            lon0 = -math.pi / 2 + math.pi * column / columns
            lon1 = -math.pi / 2 + math.pi * (column + 1) / columns
            gap_lat = (lat1 - lat0) * 0.08
            gap_lon = (lon1 - lon0) * 0.08
            corners = []
            for lat, lon in (
                (lat0 + gap_lat, lon0 + gap_lon),
                (lat0 + gap_lat, lon1 - gap_lon),
                (lat1 - gap_lat, lon1 - gap_lon),
                (lat1 - gap_lat, lon0 + gap_lon),
            ):
                x = math.cos(lat) * math.sin(lon)
                y = math.sin(lat)
                z = math.cos(lat) * math.cos(lon)
                if z <= 0.02:
                    break
                corners.append((cx + x * radius, cy - y * radius))
            else:
                mid_lon = (lon0 + lon1) / 2
                nx = math.cos(mid_lat) * math.sin(mid_lon)
                ny = math.sin(mid_lat)
                nz = math.cos(mid_lat) * math.cos(mid_lon)
                diffuse, specular = shade(nx, -ny, nz)
                # A little per tile variation, deterministic.
                wobble = ((row * 7 + column * 13) % 9) / 9 - 0.5
                base = 0.30 + 0.70 * diffuse + wobble * 0.10
                r = int(min(255, 20 + 60 * base + 220 * specular))
                g = int(min(255, 70 + 170 * base + 180 * specular))
                b = int(min(255, 30 + 80 * base + 200 * specular))
                draw.polygon(corners, fill=(r, g, b, 255))
    # Rim darkening so the ball reads as round.
    rim = Image.new("L", (size, size), 0)
    rim_draw = ImageDraw.Draw(rim)
    rim_draw.ellipse((cx - radius, cy - radius, cx + radius, cy + radius), fill=255)
    rim = rim.filter(ImageFilter.GaussianBlur(size * 0.045))
    shadow = Image.new("RGBA", (size, size), (0, 10, 4, 0))
    shadow_alpha = Image.eval(rim, lambda v: int((255 - v) * 0.9))
    sphere_mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(sphere_mask).ellipse((cx - radius, cy - radius, cx + radius, cy + radius), fill=255)
    shadow_alpha = Image.composite(shadow_alpha, Image.new("L", (size, size), 0), sphere_mask)
    shadow.putalpha(shadow_alpha)
    image.alpha_composite(shadow)
    return image, cx, cy, radius


def hook(image, cx, cy, radius):
    """The hanging hook, at the bottom: the ball is upside down."""
    draw = ImageDraw.Draw(image)
    size = image.size[0]
    w = size * 0.045
    top = cy + radius - size * 0.01
    draw.rounded_rectangle(
        (cx - w / 2, top, cx + w / 2, top + size * 0.055),
        radius=w / 3,
        fill=(150, 160, 155, 255),
    )
    ring = size * 0.028
    ring_cy = top + size * 0.055 + ring * 0.9
    draw.ellipse(
        (cx - ring, ring_cy - ring, cx + ring, ring_cy + ring),
        outline=(185, 195, 190, 255),
        width=int(size * 0.011),
    )


def sparkle(image, x, y, length, thickness):
    """A four point star of light."""
    draw = ImageDraw.Draw(image)
    glow = Image.new("RGBA", image.size, (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    glow_draw.ellipse((x - length * 0.5, y - length * 0.5, x + length * 0.5, y + length * 0.5), fill=(255, 255, 255, 110))
    glow = glow.filter(ImageFilter.GaussianBlur(length * 0.25))
    image.alpha_composite(glow)
    for angle, scale in ((0, 1.0), (90, 1.0), (45, 0.42), (135, 0.42)):
        a = math.radians(angle)
        dx, dy = math.cos(a) * length * scale, math.sin(a) * length * scale
        px, py = -math.sin(a) * thickness, math.cos(a) * thickness
        draw.polygon(
            [(x + dx, y + dy), (x + px, y + py), (x - dx, y - dy), (x - px, y - py)],
            fill=(255, 255, 255, 255),
        )


def main():
    ball, cx, cy, radius = mirror_ball(S)
    hook(ball, cx, cy, radius)
    sparkle(ball, cx + radius * 0.62, cy - radius * 0.66, S * 0.085, S * 0.006)
    sparkle(ball, cx - radius * 0.78, cy + radius * 0.35, S * 0.045, S * 0.004)
    transparent = ball.resize((SIZE, SIZE), Image.LANCZOS)
    transparent.save(HERE / "spindle-1024.png")

    # macOS: a dark rounded square behind the ball, in Apple's proportions
    # (the icon fills about 80 percent of the canvas).
    mac = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    inset = int(S * 0.10)
    ImageDraw.Draw(mac).rounded_rectangle(
        (inset, inset, S - inset, S - inset), radius=int(S * 0.18), fill=(10, 14, 12, 255)
    )
    small = ball.resize((int(S * 0.78), int(S * 0.78)), Image.LANCZOS)
    offset = (S - small.size[0]) // 2
    mac.alpha_composite(small, (offset, offset))
    mac.resize((SIZE, SIZE), Image.LANCZOS).save(HERE / "spindle-mac-1024.png")
    print("wrote", HERE / "spindle-1024.png", "and", HERE / "spindle-mac-1024.png")


if __name__ == "__main__":
    main()
