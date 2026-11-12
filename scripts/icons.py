"""`python scripts/icons.py` — the console's icons (CONSOLE.md §3). Needs Python 3 with Pillow.

The owner's pick, 2026-11-11: DeetsMusic's app icon (his head on the Skyline1 city), with the
head from his standing sprite (no headphones), and a white H in a smooth white ring over the
whole icon, the art inside the ring darkened so the H reads at 16 px.

Sources (console/icon-template/, copied from DeetsMusic):
- deets-city-600.png: the 600 px city art (DeetsMusic's icon-template/). Its own head, with
  headphones, is baked in; the sprite head below covers it.
- deets-idle-down.png: his standing sprite (DeetsMusic's src/assets/sprites/deets/idle_down.png);
  its head is the 16 x 15 pixels from the hair to the neck.

The art is built at 1024 px on whole cells (the crop is DeetsMusic's city(1024) in its
scripts/icons.py), so the face stays sharp; the ring and the H are drawn at 4x and shrunk, so
their edges are smooth. Each smaller size is the 1024 frame shrunk with a box filter.

Writes console/public/icons/: icon-512.png, icon-256.png, icon-192.png (the web-app manifest),
icon-32.png (the browser tab).
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "console/icon-template"
OUT = ROOT / "console/public/icons"
N = 1024
WHITE = (255, 255, 255, 255)

# ---- the art: the city, then the sprite head over the old head
art = Image.new("RGBA", (600, 700), (0, 0, 0, 255))  # the art's black ground carried on below
art.alpha_composite(Image.open(SRC / "deets-city-600.png").convert("RGBA"))
X0, Y0, SPAN = 60, 45, 480  # DeetsMusic's city(1024) crop
icon = art.crop((X0, Y0, X0 + SPAN, Y0 + SPAN)).resize((N, N), Image.NEAREST)

sprite = Image.open(SRC / "deets-idle-down.png").convert("RGBA")
head = sprite.crop((8, 5, 24, 20))  # 16 x 15: hair to neck, the collar row left out
CELL = 48  # 16 x 48 = 768 px: as wide as the old head with its ear cups, so it covers them
head = head.resize((16 * CELL, 15 * CELL), Image.NEAREST)
OLD_BOTTOM = round((165 + 15 * 20 - Y0) * N / SPAN)  # the old head's bottom edge (20 px cells at y 165)
icon.alpha_composite(head, ((N - head.width) // 2, OLD_BOTTOM - head.height))

# ---- the mark: a dark disc, a smooth white ring, the H (all at 4x, then shrunk)
S = 4
mark = Image.new("RGBA", (N * S, N * S), (0, 0, 0, 0))
d = ImageDraw.Draw(mark)
MARGIN, RING = 26, 56  # the ring's outer edge from the icon's edge, and its width
DIM = 150  # the darkening inside the ring (alpha of black, of 255)
d.ellipse(((MARGIN + RING // 2) * S,) * 2 + ((N - MARGIN - RING // 2) * S,) * 2, fill=(0, 0, 0, DIM))
d.ellipse((MARGIN * S,) * 2 + ((N - MARGIN) * S,) * 2, outline=WHITE, width=RING * S)
# The H on the 16 px grid of the first trials: posts 96 wide, 480 tall; the bar 224 long.
TOP, BOTTOM, LEFT, RIGHT, POST, BAR_TOP, BAR_BOTTOM = 272, 752, 304, 720, 96, 464, 560
d.rectangle((LEFT * S, TOP * S, (LEFT + POST) * S - 1, BOTTOM * S - 1), fill=WHITE)
d.rectangle(((RIGHT - POST) * S, TOP * S, RIGHT * S - 1, BOTTOM * S - 1), fill=WHITE)
d.rectangle((LEFT * S, BAR_TOP * S, RIGHT * S - 1, BAR_BOTTOM * S - 1), fill=WHITE)
icon.alpha_composite(mark.resize((N, N), Image.LANCZOS))

OUT.mkdir(parents=True, exist_ok=True)
for size in (512, 256, 192, 32):
    icon.resize((size, size), Image.BOX).save(OUT / f"icon-{size}.png")
print(f"[icons] icon-512, -256, -192, -32 in {OUT.relative_to(ROOT).as_posix()}/")
