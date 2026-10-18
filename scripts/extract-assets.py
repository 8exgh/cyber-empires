#!/usr/bin/env python3
"""Copy the original Cyber Empires data files into web/game/ (git-ignored) so the browser build can load them.

Usage: scripts/extract-assets.py [path-to-zip-or-folder]   (default: original/*.zip)
"""
import os, sys, zipfile, glob

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
out = os.path.join(root, 'web', 'game')
src = sys.argv[1] if len(sys.argv) > 1 else (glob.glob(os.path.join(root, 'original', '*.zip')) or [None])[0]
if not src:
    sys.exit("no source given and no zip found in original/")
exts = ('.gif', '.map', '.dat', '.iat', '.bin', '.mus')
os.makedirs(out, exist_ok=True)
names = []
if os.path.isdir(src):
    for dp, _, fs in os.walk(src):
        for f in fs:
            if f.lower().endswith(exts):
                data = open(os.path.join(dp, f), 'rb').read()
                open(os.path.join(out, f.lower()), 'wb').write(data); names.append(f.lower())
else:
    with zipfile.ZipFile(src) as z:
        for info in z.infolist():
            base = os.path.basename(info.filename)
            if base.lower().endswith(exts) and not info.is_dir():
                open(os.path.join(out, base.lower()), 'wb').write(z.read(info)); names.append(base.lower())
names.sort()
open(os.path.join(out, 'manifest.txt'), 'w').write('\n'.join(names) + '\n')
print(f"extracted {len(names)} files to {out}")
