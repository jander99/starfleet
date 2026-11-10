#!/usr/bin/env python3
"""Turn the raw Earth frames in art/earth/ into app assets in src/assets/earth/.

The frames were generated as one scene lit four ways (day, dusk, night, dawn).
Their deep-space corner is pure black; this makes that connected region
transparent so stars can show through. Needs Pillow: pip install pillow
"""
from collections import deque
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SRC, DST = ROOT / "art/earth", ROOT / "src/assets/earth"
SPACE = 3  # frames have near-black (<= 3) noise in space
DST.mkdir(parents=True, exist_ok=True)

for path in sorted(SRC.glob("earth-*.png")):
    im = Image.open(path).convert("RGBA")
    w, h = im.size
    px = im.load()
    seen = [[False] * w for _ in range(h)]
    queue = deque([(w - 1, 0)])
    while queue:
        x, y = queue.popleft()
        if not (0 <= x < w and 0 <= y < h) or seen[y][x] or max(px[x, y][:3]) > SPACE:
            continue
        seen[y][x] = True
        px[x, y] = (0, 0, 0, 0)
        queue.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)])
    im.save(DST / path.name, optimize=True)
    limb = {x: next(y for y in range(h) if px[x, y][3]) for x in (60, 86, 150, 206, 270)}
    print(path.name, im.size, "limb y at x:", limb)
