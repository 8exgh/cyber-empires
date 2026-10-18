# Reverse-engineering tools

These scripts were used to recover the game rules from the original DOS executables. They need
`pip install iced-x86` and Pillow, and the original game files (not included in this repository).

- `disasm.py <PRG> <outprefix>` — parses the Borland Turbo C debug symbol table appended to CYBOR.PRG /
  ACTION.PRG / INTRO.PRG and writes an annotated 16-bit disassembly (`<outprefix>.asm`) plus `<outprefix>.syms.json`.
- `tables.py <syms.json> <PRG> <out.txt>` — dumps every data-segment symbol with its initial bytes decoded as
  shorts/longs/floats/strings (unit costs, build times, weapon tables, AI tables, UI rectangles...).
- `extract_world.py <gamedir>` — extracts the 72 countries (name, terrain, adjacency, map coordinates) from MECHS.DAT.
- `gif2png.py <gamedir> <outdir>` — converts every GIF/MAP (the .MAP battle maps are GIF87a images) to PNG.

The findings are summarised in `docs/`.
