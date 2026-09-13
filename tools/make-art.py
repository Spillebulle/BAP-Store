#!/usr/bin/env python3
"""The mark, the icons and the banners, from the palette rather than by hand.

    python3 tools/make-art.py

Writes assets/icons/brokey-{16,32,48,64,128,256,512}.png, brokey.ico,
the four files crates/brokey/icons/ that Tauri's bundler names, and, via
Design-Principles' banner.py (a copy is in tools/), docs/images/banner.png,
banner-paper.png and social.png.

The colour is not typed in: it is the dark theme's `--accent` for this
application's hue, `oklch(0.674 0.101 300)`, converted here with the same
arithmetic the browser uses, so a change to the hue in tokens.css and a rerun
of this script cannot disagree. The mark is a filled rounded square, corner
radius 22 % of the side, no glyph (STYLE-GUIDE.md §17.4).

Requires Pillow.
"""
from __future__ import annotations

import math
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
import banner  # noqa: E402  the house banner arithmetic, copied from Design-Principles

ROOT = Path(__file__).resolve().parent.parent
TOKENS = ROOT / "frontend/src/tokens.css"


def accent_hue() -> float:
    m = re.search(r"--accent-h:\s*([0-9.]+)", TOKENS.read_text(encoding="utf-8"))
    if not m:
        sys.exit("tokens.css has no --accent-h")
    return float(m.group(1))


def oklch_to_srgb(L: float, C: float, h: float) -> tuple[int, int, int]:
    """Björn Ottosson's OKLab, the transform CSS Color 4 specifies."""
    a = C * math.cos(math.radians(h))
    b = C * math.sin(math.radians(h))
    l_ = L + 0.3963377774 * a + 0.2158037573 * b
    m_ = L - 0.1055613458 * a - 0.0638541728 * b
    s_ = L - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_**3, m_**3, s_**3
    r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s
    g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s
    bl = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s

    def gamma(x: float) -> int:
        x = min(1.0, max(0.0, x))
        v = 1.055 * x ** (1 / 2.4) - 0.055 if x > 0.0031308 else 12.92 * x
        return int(round(v * 255))

    return gamma(r), gamma(g), gamma(bl)


def mark(size: int, colour: tuple[int, int, int], scale: int = 8) -> Image.Image:
    """Drawn large and reduced, so the corner is smooth at 16 px too."""
    big = size * scale
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(img).rounded_rectangle(
        [0, 0, big - 1, big - 1], radius=int(big * 0.22), fill=(*colour, 255)
    )
    return img.resize((size, size), Image.LANCZOS)


def main() -> int:
    hue = accent_hue()
    colour = oklch_to_srgb(0.674, 0.101, hue)
    print(f"accent hue {hue:g} -> #{colour[0]:02X}{colour[1]:02X}{colour[2]:02X}")

    icons = ROOT / "assets/icons"
    icons.mkdir(parents=True, exist_ok=True)
    sizes = [16, 32, 48, 64, 128, 256, 512]
    images = {s: mark(s, colour) for s in sizes}
    for s, img in images.items():
        img.save(icons / f"brokey-{s}.png")
    images[256].save(icons / "brokey.ico", sizes=[(s, s) for s in (16, 32, 48, 64, 128, 256)])

    tauri = ROOT / "crates/brokey/icons"
    tauri.mkdir(parents=True, exist_ok=True)
    images[32].save(tauri / "32x32.png")
    images[128].save(tauri / "128x128.png")
    images[256].save(tauri / "128x128@2x.png")
    images[512].save(tauri / "icon.png")
    print(f"wrote {len(sizes)} icon sizes, the .ico and Tauri's four")

    out = ROOT / "docs/images"
    out.mkdir(parents=True, exist_ok=True)
    font = ROOT / "assets/fonts/Archivo.ttf"
    for name, (ground, ink) in banner.GROUNDS.items():
        img = banner.compose(images[512], font, "BROKEY", ground, ink, banner.WIDTH, banner.HEIGHT)
        img.save(out / name)
        print(f"wrote {out / name}")
    ground, ink = banner.GROUNDS["banner.png"]
    banner.compose(images[512], font, "BROKEY", ground, ink, *banner.SOCIAL).save(out / "social.png")
    print(f"wrote {out / 'social.png'}")
    return 0



if __name__ == "__main__":
    sys.exit(main())
