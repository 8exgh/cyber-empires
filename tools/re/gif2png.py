import sys, os
from PIL import Image
src, dst = sys.argv[1], sys.argv[2]
for f in sorted(os.listdir(src)):
    if f.upper().endswith(('.GIF', '.MAP', '.BMP')):
        p = os.path.join(src, f)
        try:
            im = Image.open(p)
            im.load()
            pal = im.getpalette()
            ncol = len(set(im.getdata()))
            # save scaled x3 for viewing
            big = im.convert('RGB').resize((im.width*3, im.height*3), Image.NEAREST)
            big.save(os.path.join(dst, f + '.png'))
            print(f"{f:14s} {im.width}x{im.height} mode={im.mode} colors_used={ncol} palsize={len(pal)//3 if pal else 0}")
        except Exception as e:
            print(f"{f}: ERROR {e}")
