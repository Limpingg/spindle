#!/usr/bin/env python3
"""Makes every Spindle icon from Nick's picture, packaging/icons/source.png
(neon headphones and a sound wave on a dark green rounded square):

    packaging/icons/spindle-1024.png       rounded square, clear corners
                                           (window, Windows tray, in-app logo)
    packaging/icons/spindle-mac-1024.png   the same on Apple's icon grid
    packaging/macos/icon-1024.png          copy of the Mac master
    packaging/icons/spindle-tray-1024.png  the neon lines alone, for the
                                           macOS menu bar template
    packaging/icons/spindle-small-256.png  the small drawing (below), which
                                           the app scales for 40 px and under
    packaging/icons/spindle.svg            Linux tray (the PNG inside an SVG)
    packaging/windows/spindle.ico          PNG entries, 16 to 256 (16, 24
                                           and 32 are the small drawing)
    packaging/macos/spindle.icns           icp4..ic14
    installer/assets/window-icon.png       64 px, the setup program's window

The picture's thin neon lines run into a round green blob at 16 to 32 px
(the title bar, the taskbar, Task Manager, the tray), which read as the old
mirror ball. Those sizes get a simpler drawing of the same thing instead:
the dark green square, the neon headband and ear cups, and a short wave.

Run from the repository root: python3 packaging/icons/make-icons.py
"""

import base64
import io
import struct
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
ICONS = ROOT / "packaging/icons"
SOURCE = ICONS / "source.png"
# The rounded square inside the source picture, cut square.
CROP = (207, 56, 819, 668)
RADIUS = 0.225  # corner radius as a share of the side, close to Apple's


def rounded_mask(side: int) -> Image.Image:
    big = side * 4
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, big - 1, big - 1), radius=int(big * RADIUS), fill=255)
    return mask.resize((side, side), Image.LANCZOS)


def body(side: int) -> Image.Image:
    art = Image.open(SOURCE).convert("RGB").crop(CROP).resize((side, side), Image.LANCZOS)
    tile = art.convert("RGBA")
    tile.putalpha(rounded_mask(side))
    return tile


def placed(side: int, inset: int) -> Image.Image:
    canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    canvas.paste(body(side), (inset, inset))
    return canvas


def lines(side: int = 1024) -> Image.Image:
    """The neon lines on clear: alpha from how far the green rises above
    the square's own dark green."""
    art = Image.open(SOURCE).convert("RGB").crop(CROP).resize((side, side), Image.LANCZOS)
    green = art.getchannel("G")
    alpha = green.point(lambda g: 0 if g < 110 else min(255, (g - 110) * 255 // 90))
    alpha = ImageChops.multiply(alpha, rounded_mask(side))
    out = Image.new("RGBA", (side, side), (40, 255, 120, 0))
    out.putalpha(alpha)
    return out


BACKGROUND = (6, 34, 18, 255)
NEON = (70, 255, 125, 255)
SMALL = (16, 24, 32)


def small(size: int) -> Image.Image:
    """The picture simplified for `size` px: headband, ear cups and a wave,
    drawn on a 32 unit grid at 16 times the size, then scaled down."""
    scale = 16
    side = size * scale
    image = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((0, 0, side - 1, side - 1), radius=int(side * RADIUS), fill=BACKGROUND)
    unit = side / 32
    width = int((2.6 if size >= 24 else 2.4) * unit)
    draw.arc((5 * unit, 5 * unit, 27 * unit, 29 * unit), start=180, end=360, fill=NEON, width=width)
    for x in (5 * unit + width / 2, 27 * unit - width / 2):
        draw.line((x, 17 * unit, x, 19 * unit), fill=NEON, width=width)
    for left, right in ((3.5, 9), (23, 28.5)):
        draw.rounded_rectangle((left * unit, 17 * unit, right * unit, 26.5 * unit), radius=int(2 * unit), fill=NEON)
    if size >= 24:
        bar = 1.6 * unit
        for x, height in ((11.5, 2.5), (14, 5.5), (16.5, 3.5), (19, 6), (21, 2)):
            draw.rounded_rectangle(
                (x * unit - bar / 2, (22 - height / 2) * unit, x * unit + bar / 2, (22 + height / 2) * unit),
                radius=int(bar / 2),
                fill=NEON,
            )
    return image.resize((size, size), Image.LANCZOS)


def ico(master: Image.Image, path: Path) -> None:
    sizes = [16, 24, 32, 48, 64, 128, 256]
    entries = []
    for size in sizes:
        buffer = io.BytesIO()
        picture = small(size) if size in SMALL else master.resize((size, size), Image.LANCZOS)
        picture.save(buffer, "PNG")
        entries.append((size, buffer.getvalue()))
    header = struct.pack("<HHH", 0, 1, len(entries))
    offset = 6 + 16 * len(entries)
    directory = b""
    data = b""
    for size, png in entries:
        directory += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(png), offset + len(data))
        data += png
    path.write_bytes(header + directory + data)


def icns(master: Image.Image, path: Path) -> None:
    chunks = [("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic10", 1024),
              ("ic11", 32), ("ic12", 64), ("ic13", 256), ("ic14", 512)]
    body_bytes = b""
    for kind, size in chunks:
        buffer = io.BytesIO()
        master.resize((size, size), Image.LANCZOS).save(buffer, "PNG")
        png = buffer.getvalue()
        body_bytes += kind.encode() + struct.pack(">I", len(png) + 8) + png
    path.write_bytes(b"icns" + struct.pack(">I", len(body_bytes) + 8) + body_bytes)


def svg(master: Image.Image, path: Path) -> None:
    buffer = io.BytesIO()
    master.resize((128, 128), Image.LANCZOS).save(buffer, "PNG", optimize=True)
    data = base64.b64encode(buffer.getvalue()).decode()
    path.write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
        'viewBox="0 0 128 128" width="128" height="128">\n'
        "  <!-- Spindle: neon headphones and a sound wave. A plain embedded PNG, no filters, for Qt's tray renderer. -->\n"
        f'  <image width="128" height="128" xlink:href="data:image/png;base64,{data}"/>\n'
        "</svg>\n"
    )


def main() -> None:
    master = placed(976, 24)
    mac = placed(824, 100)
    master.save(ICONS / "spindle-1024.png", optimize=True)
    mac.save(ICONS / "spindle-mac-1024.png", optimize=True)
    mac.save(ROOT / "packaging/macos/icon-1024.png", optimize=True)
    lines().save(ICONS / "spindle-tray-1024.png", optimize=True)
    small(256).save(ICONS / "spindle-small-256.png", optimize=True)
    svg(master, ICONS / "spindle.svg")
    ico(master, ROOT / "packaging/windows/spindle.ico")
    icns(mac, ROOT / "packaging/macos/spindle.icns")
    master.resize((64, 64), Image.LANCZOS).save(ROOT / "installer/assets/window-icon.png", optimize=True)
    print("icons written")


if __name__ == "__main__":
    main()
