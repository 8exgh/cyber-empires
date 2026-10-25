#!/usr/bin/env python3
"""Build src/ui/credit.gif, the "ported by" screen shown before the original logos.

Usage: scripts/make-credit.py <avatar image>   (the @8examples profile picture)

The game draws 320x200 pixels on a 4:3 display, so each pixel is 1.2x taller than wide;
the portrait is squashed vertically to look square on screen. The image is quantised
to 240 colours: palette slots 240..255 stay free for the in-game font colours, which
the game draws over the image at runtime ("PORTED BY" / "SEAN BENNETT").
"""
import random, sys
from PIL import Image, ImageDraw

W, H = 320, 200
AV_W = 128
AV_H = round(AV_W / 1.2)            # 107: square once stretched to 4:3
AV_X = (W - AV_W) // 2              # 96
AV_Y = 22
out = "src/ui/credit.gif"

src = Image.open(sys.argv[1]).convert("RGB")
side = min(src.size)
src = src.crop(((src.width - side) // 2, (src.height - side) // 2,
                (src.width + side) // 2, (src.height + side) // 2))
avatar = src.resize((AV_W, AV_H), Image.LANCZOS)

img = Image.new("RGB", (W, H), (0, 0, 0))
d = ImageDraw.Draw(img)
rng = random.Random(1992)
for _ in range(90):                 # sparse starfield in the SSI logo's blues
    x, y = rng.randrange(W), rng.randrange(H)
    if AV_X - 8 <= x <= AV_X + AV_W + 8 and AV_Y - 8 <= y <= AV_Y + AV_H + 8:
        continue
    c = rng.choice([(40, 60, 140), (70, 110, 220), (120, 170, 255), (200, 220, 255)])
    d.point((x, y), fill=c)
    if rng.random() < 0.12:         # a few twinkles
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            d.point((x + dx, y + dy), fill=(c[0] // 2, c[1] // 2, c[2] // 2))
# two-tone frame in the title screen's purples
d.rectangle((AV_X - 3, AV_Y - 3, AV_X + AV_W + 2, AV_Y + AV_H + 2), fill=(60, 30, 120))
d.rectangle((AV_X - 2, AV_Y - 2, AV_X + AV_W + 1, AV_Y + AV_H + 1), fill=(185, 160, 255))
d.rectangle((AV_X - 1, AV_Y - 1, AV_X + AV_W, AV_Y + AV_H), fill=(30, 15, 60))
img.paste(avatar, (AV_X, AV_Y))

q = img.quantize(colors=240, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.FLOYDSTEINBERG)
pal = q.getpalette()[:240 * 3]
q.putpalette(pal + [0] * (256 * 3 - len(pal)))
assert max(q.getdata()) < 240
q.save(out, optimize=False)
print(f"wrote {out}")
