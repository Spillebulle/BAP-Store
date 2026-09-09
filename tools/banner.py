#!/usr/bin/env python3
"""The house banner, as a measurement rather than a judgement (§17.4).

Two jobs, one set of numbers:

    python banner.py --check ../Umber/docs/images/banner.png --name UMBER
    python banner.py --mark ../Muster/assets/icons/muster-256.png --name MUSTER \
                     --out ../Muster/docs/images

`--check` measures a banner that already exists and prints every dimension
beside the one the guide asks for. `--mark` draws a new pair from the app's own
mark artwork. Neither invents a colour: the grounds are `--backdrop` and the ink
is `--text-strong`, read out of `tokens.css`.

The geometry is §17.4's table. It was not chosen, it was measured: Umber, Tally,
Trove and Muster's banners were taken apart pixel by pixel and these are the
numbers they agree on. Umber's lands exactly on the width limit, which is why
the limit is where it is.

Requires Pillow.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent

# ── The table ────────────────────────────────────────────────────────────────

WIDTH, HEIGHT = 1354, 461      # the canvas, Umber's, in every repository
SOCIAL = (1280, 640)           # GitHub's social preview, §17.2

MARK_PER_HEIGHT = 0.34         # mark side, as a fraction of the banner height
MARK_PER_CAP = 1.20            # mark side, as a multiple of the wordmark's cap
GAP_PER_CAP = 0.46             # mark edge to the first letter's ink
GROUP_PER_WIDTH = 0.70         # the pair may occupy this much of the width
MIN_SCALE = 0.75               # how far the group may shrink before it stacks
TRACKING_EM = -0.031           # −2 px at a 64 px em, stated per em so it scales

# Archivo Black: the cap height, and how far the round letters overshoot it.
CAP_EM = 0.688
OVERSHOOT_EM = 0.712
ROUND = set("OQCGSUJ")         # the letters that reach past the cap line

# `--backdrop` and `--text-strong`, both themes, from tokens.css.
GROUNDS = {
    "banner.png": ("#0D0E10", "#E6E7E9"),
    "banner-paper.png": ("#E4E0D9", "#3A3836"),
}

# How far a measured banner may sit from the table. The proportions inside the
# group are tight; the gap is the loosest, because half a letter of air either
# way is a judgement the eye forgives and the family has never agreed on.
TOLERANCE = {
    "mark / height": 0.02,
    "mark / cap": 0.06,
    "gap / cap": 0.12,
    "group / width": 0.02,
}
AT_LIMIT = 0.03                # near enough to a limit to count as bound by it
LEAN = 6                       # px the group may sit off the horizontal centre


# ── The typeface ─────────────────────────────────────────────────────────────

def load_font(path: Path, px: float) -> ImageFont.FreeTypeFont:
    """Archivo at Weight 900, Width 100.

    The variation axes are the whole point: ask a variable font for a bold
    without setting them and you get the Regular master back, quietly.
    """
    font = ImageFont.truetype(str(path), max(1, int(round(px))))
    try:
        font.set_variation_by_axes([900, 100])
    except OSError:
        pass                                     # a static Black face, already there
    return font


def advance(font: ImageFont.FreeTypeFont, text: str, tracking: float) -> float:
    return sum(font.getlength(c) for c in text) + tracking * (len(text) - 1)


def word_layer(font_path: Path, text: str, cap: float, ink: str):
    """The wordmark, cropped to its ink, plus where its baseline sits inside it.

    Cropping to the ink is what makes the two margins equal. Centring the
    advance box instead leaves the last letter's side bearing in the picture,
    which is the four pixels by which every banner in the family currently
    leans left.
    """
    em = cap / CAP_EM
    font = load_font(font_path, em)
    tracking = TRACKING_EM * em
    pad = int(em)
    w = int(advance(font, text, tracking) + 2 * pad)
    h = int(em * 3)
    layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    baseline = h * 0.6
    x = float(pad)
    for char in text:
        draw.text((x, baseline), char, font=font, fill=ink, anchor="ls")
        x += font.getlength(char) + tracking
    box = layer.getchannel("A").getbbox()
    return layer.crop(box), baseline - box[1]


def ink_width(font_path: Path, text: str, cap: float) -> float:
    return word_layer(font_path, text, cap, "#FFFFFF")[0].width


# ── Drawing ──────────────────────────────────────────────────────────────────

def fit(font_path: Path, text: str):
    """The one calculation: the cap height this name gets, and how far it shrank.

    Height sets the mark, width caps the pair, and the smaller wins. Measuring
    at a probe size and scaling keeps it one calculation rather than a search.
    It is done against the banner's canvas and nothing else: the social preview
    is a different shape, and sizing the group to *it* would give one repository
    two brand groups at two sizes.
    """
    cap_full = HEIGHT * MARK_PER_HEIGHT / MARK_PER_CAP

    probe = 100.0
    probe_group = (probe * MARK_PER_CAP + probe * GAP_PER_CAP
                   + ink_width(font_path, text, probe))
    cap_by_width = probe * (WIDTH * GROUP_PER_WIDTH) / probe_group

    cap = min(cap_full, cap_by_width)
    return cap, cap / cap_full


def compose(mark_src: Image.Image, font_path: Path, text: str,
            ground: str, ink: str, width: int, height: int) -> Image.Image:
    cap, scale = fit(font_path, text)
    if scale < MIN_SCALE:
        raise SystemExit(
            f"'{text}' only fits at {scale:.2f} of full size, under the {MIN_SCALE:.2f} "
            f"floor. A name this long sets on two lines rather than shrinking the "
            f"whole group to suit it (see 17.4).")

    mark_h = int(round(cap * MARK_PER_CAP))
    mark_w = int(round(mark_src.width * mark_h / mark_src.height))
    gap = cap * GAP_PER_CAP

    word, baseline_in_word = word_layer(font_path, text, cap, ink)
    group_w = mark_w + gap + word.width
    left = (width - group_w) / 2
    middle = height / 2

    canvas = Image.new("RGB", (width, height), ground)
    mark = mark_src.resize((mark_w, mark_h), Image.LANCZOS)
    canvas.paste(mark, (int(round(left)), int(round(middle - mark_h / 2))), mark)

    # The mark stands a fifth taller than the caps, so the two share a centre
    # line rather than a top edge: the cap band is centred on the middle, which
    # puts the baseline half a cap below it.
    baseline = middle + cap / 2
    canvas.paste(word, (int(round(left + mark_w + gap)),
                        int(round(baseline - baseline_in_word))), word)
    return canvas


# ── Measuring ────────────────────────────────────────────────────────────────

def measure(path: Path, name: str | None) -> dict:
    im = Image.open(path).convert("RGB")
    px = im.load()
    w, h = im.size
    bg = px[0, 0]

    def ink(x, y):
        c = px[x, y]
        return abs(c[0] - bg[0]) + abs(c[1] - bg[1]) + abs(c[2] - bg[2]) > 24

    cols = [any(ink(x, y) for y in range(h)) for x in range(w)]
    if not any(cols):
        raise SystemExit(f"{path}: nothing drawn on it")
    x0 = cols.index(True)
    x1 = w - 1 - cols[::-1].index(True)

    # The widest run of empty columns inside the content is the mark/word gap.
    runs, start = [], None
    for x in range(x0, x1 + 1):
        if not cols[x]:
            start = x if start is None else start
        elif start is not None:
            runs.append((start, x - 1))
            start = None
    if not runs:
        raise SystemExit(f"{path}: no gap between mark and wordmark")
    gs, ge = max(runs, key=lambda r: r[1] - r[0])

    def band(xa, xb):
        rows = [y for y in range(h) if any(ink(x, y) for x in range(xa, xb + 1))]
        return rows[0], rows[-1]

    my0, my1 = band(x0, gs - 1)
    wy0, wy1 = band(ge + 1, x1)

    ink_h = wy1 - wy0 + 1
    # Round letters overshoot the cap line, so the wordmark's ink is taller
    # than its cap height whenever the name has one in it.
    top = OVERSHOOT_EM if name and set(name) & ROUND else CAP_EM
    cap = ink_h * CAP_EM / top

    return dict(
        width=w, height=h, ground="#%02X%02X%02X" % bg,
        mark=my1 - my0 + 1, cap=cap, gap=ge - gs + 1,
        group=x1 - x0 + 1, left=x0, right=w - 1 - x1,
        mark_mid=(my0 + my1) / 2, word_mid=(wy0 + wy1) / 2,
        corrected=bool(name and set(name) & ROUND),
    )


def check(path: Path, name: str | None) -> bool:
    m = measure(path, name)
    h, w = m["height"], m["width"]
    # The social preview carries the banner's group on a different frame, so its
    # size is judged against the banner's canvas, not against its own.
    frame_w, frame_h = (WIDTH, HEIGHT) if (w, h) == SOCIAL else (w, h)
    mark_h = m["mark"] / frame_h
    group_w = m["group"] / frame_w

    print(f"\n{path}")
    print(f"  canvas {w} x {h}   ground {m['ground']}"
          f"   mark {m['mark']} px   cap {m['cap']:.0f} px"
          f"   gap {m['gap']} px   margins {m['left']} / {m['right']}")
    if m["corrected"]:
        print("  (cap corrected for the round letters' overshoot)")

    lines, ok = [], True

    def report(label, value, passed, want):
        nonlocal ok
        ok &= passed
        lines.append(f"  {'ok  ' if passed else 'OFF '} {label:<17} {value}   want {want}")

    # Proportions inside the group. These hold whatever size the group ends up.
    for label, value, target in (("mark / cap", m["mark"] / m["cap"], MARK_PER_CAP),
                                 ("gap / cap", m["gap"] / m["cap"], GAP_PER_CAP)):
        slack = TOLERANCE[label]
        report(label, f"{value:.3f}", abs(value - target) <= slack,
               f"{target:.2f} +/- {slack:.2f}")

    # The group's size. Height sets the mark, width caps the pair, the smaller
    # wins, and whichever wins has to actually be reached: a group under both
    # limits is one that was fitted to nothing in particular.
    report("mark / height", f"{mark_h:.3f}",
           mark_h <= MARK_PER_HEIGHT + TOLERANCE["mark / height"],
           f"<= {MARK_PER_HEIGHT:.2f}")
    report("group / width", f"{group_w:.3f}",
           group_w <= GROUP_PER_WIDTH + TOLERANCE["group / width"],
           f"<= {GROUP_PER_WIDTH:.2f}")
    bound = (abs(mark_h - MARK_PER_HEIGHT) <= AT_LIMIT
             or abs(group_w - GROUP_PER_WIDTH) <= AT_LIMIT)
    which = ("full size" if abs(mark_h - MARK_PER_HEIGHT) <= AT_LIMIT
             else "width-bound" if bound else "under both limits")
    report("at its limit", f"{which:<9}", bound, "height or width")
    scale = mark_h / MARK_PER_HEIGHT
    report("shrunk to", f"{scale:.2f}", scale >= MIN_SCALE - 0.01,
           f">= {MIN_SCALE:.2f}, else two lines")

    # Where the group sits. All three are about the eye, not the arithmetic.
    report("mark centred", f"{m['mark_mid']:.1f}",
           abs(m["mark_mid"] - h / 2) <= 1.5, f"{h / 2:.1f}")
    report("cap band centred", f"{m['word_mid']:.1f}",
           abs(m["word_mid"] - h / 2) <= 3.0, f"{h / 2:.1f}")
    report("margins equal", f"{m['left']} / {m['right']}",
           abs(m["left"] - m["right"]) <= LEAN, f"within {LEAN} px")

    if (w, h) not in ((WIDTH, HEIGHT), SOCIAL):
        report("canvas", f"{w} x {h}", False, f"{WIDTH} x {HEIGHT}")
    if m["ground"] not in (GROUNDS["banner.png"][0], GROUNDS["banner-paper.png"][0]):
        report("ground", m["ground"], False, "a --backdrop")

    print("\n".join(lines))
    return ok


# ── Entry ────────────────────────────────────────────────────────────────────

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", type=Path, nargs="+", metavar="PNG",
                    help="measure banners that already exist")
    ap.add_argument("--mark", type=Path, help="the app's mark, PNG with alpha")
    ap.add_argument("--name", help="the wordmark, e.g. MUSTER")
    ap.add_argument("--out", type=Path, default=Path("docs/images"))
    ap.add_argument("--font", type=Path, default=HERE / "fonts/Archivo.ttf")
    ap.add_argument("--social", action="store_true",
                    help="also write social-preview.png at 1280 x 640")
    args = ap.parse_args()

    if args.check:
        every = True
        for path in args.check:
            every &= check(path, args.name.upper() if args.name else None)
        print()
        return 0 if every else 1

    if not (args.mark and args.name):
        ap.error("give --check, or --mark and --name")
    if not args.mark.exists():
        return print(f"Missing: {args.mark}", file=sys.stderr) or 1
    if not args.font.exists():
        return print(f"Missing: {args.font}", file=sys.stderr) or 1

    name = args.name.upper()
    mark_src = Image.open(args.mark).convert("RGBA")
    # Trim the artwork's own transparent border, so the mark's drawn size is the
    # size this script sets and not whatever padding the source file carries.
    box = mark_src.getchannel("A").getbbox()
    if box:
        mark_src = mark_src.crop(box)

    args.out.mkdir(parents=True, exist_ok=True)
    for file, (ground, ink) in GROUNDS.items():
        target = args.out / file
        compose(mark_src, args.font, name, ground, ink, WIDTH, HEIGHT).save(target, "PNG")
        print(f"Wrote {target} ({WIDTH} x {HEIGHT})")
    if args.social:
        target = args.out / "social-preview.png"
        dark, ink = GROUNDS["banner.png"]
        compose(mark_src, args.font, name, dark, ink, *SOCIAL).save(target, "PNG")
        print(f"Wrote {target} ({SOCIAL[0]} x {SOCIAL[1]})")

    cap, scale = fit(args.font, name)
    print(f"cap {cap:.0f} px, mark {cap * MARK_PER_CAP:.0f} px, "
          f"gap {cap * GAP_PER_CAP:.0f} px, group at {scale:.2f} of full size")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
