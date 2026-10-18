# Cyber Empires (SSI/Silicon Knights 1992, DOS) reverse-engineering context

Goal: re-implement the game faithfully in Rust. These notes are what is known so far.
All files are in /tmp/claude-1000/-home-sean-8Examples-cyber-empires/14cd31d7-310c-4a08-af09-6aceedf33860/scratchpad/ :
- orig/cyber-empires/cyber/   original game files (DO NOT execute anything from here)
- cybor.asm / action.asm / intro.asm   annotated 16-bit disassembly (NASM syntax, iced-x86) of CYBOR.PRG (strategy layer),
  ACTION.PRG (real-time battle), INTRO.PRG (intro/options/copy-protection). Every function starts with a line
  `;=== _NAME  seg=... off=... addr=... len=...`. Calls are annotated with the target symbol; memory operands with
  DGROUP data symbol names like `[_MECHRECORD+386]` or `[+_UNITCOST]` (the latter = indexed access base). Immediates that
  point to strings are annotated `&'text'`.
- *.syms.json  symbol tables; re_cybor_tables.txt / re_action_tables.txt / re_intro_tables.txt  all data-segment
  symbols with initial bytes decoded as shorts/longs/floats/strings.
- png/  every GIF converted to PNG (3x scaled) for viewing.
- world.json  the 72 countries (name, terrain, adjacency, map x/y).

Toolchain facts: Turbo C 2.0, large/medium memory model (far calls `call seg:off`), DS=DGROUP. int=16-bit, long=32-bit
(little-endian in memory, low word at lower address). Turbo C `float` is IEEE single. Turbo C passes args on the stack
right-to-left; [bp+6] is first arg for far functions ([bp+4] for near). Return value in AX (DX:AX for long).
_RAND_NUM(lo, hi) returns a random int in [lo, hi] inclusive (verify in asm: args pushed hi then lo → first arg lo).
Data files are stored BIG-ENDIAN (Atari ST heritage); _CINT/_LINT/_REVERSE byte-swap on load/save.
GEM/VDI-named helpers (_VSF_COLOR, _VR_RECFL, _EVNT_BUTTON, _GRAF_MKSTATE, _RECT_SET) are just graphics/mouse primitives.

## Main game struct `_MECHRECORD` (17652 bytes, == GAME?.DAT / MECHS.DAT / ACTS.DAT file image)
Offsets relative to _MECHRECORD:
- +0 short  number of players (5)
- +2 short  ?
- +4 short  planet factor (1 poor, 2 average, 3 rich) used as multiplier for country values
- +6 short  (96 in saves), +8 short (200 in saves), +10 short (0)
- +12.. five player records of 32 bytes each (player p at 12+32*p):
    +0 short id (0..4), +2 char name[16], +18 short f30?, +20 short?, ... In _GAME_INIT: [+20]=0x7690 and [+22]=0x12 → long at +20 = 0x00127690 = 1,209,968 (starting vault / money?) — verify.
    +26 / +28 shorts = map scroll x/y at start. Exact semantics of each player field must be determined.
- +172 .. country records, 236 (0xEC) bytes each, country number c (1..72) is at _MECHRECORD + c*236 (so country 1 at +236; the bytes 172..235 are country "0", unused/garbage).
  Country record layout (offsets relative to the record start c*236):
    +172 short owner (player id 0..4, 5 = neutral)      [NOTE: offsets below 172 belong to the previous record]
    +174 char name[14]
    +188 short (0)
    +190 short country number
    +192 short government/capital established flag (1/0)
    +194,+196,+198..+204: shorts f3,f4,f5,f6,f7,f8 (meaning unknown: fort/factory/enhancement related — determine!)
    +206..+222: 9 shorts adjacency list (country numbers, 0-terminated)
    +224 short  set to rand(2,7) at init, 1 for cities ("build space"?)
    +226 short  fortification level 0..3 ?
    +228 short  number of cyborgs present
    +230,+232 shorts map x,y of the country icon on the 640x400 world map
    +234..+333: 10 cyborg slots of 5 shorts: [type (0..8, 10=empty), status/health %, home country number, ?, ?]
    +334..+381: 24 shorts factory bays (10 = empty; values like 104, 302, 102 seen = type*100 + turns-left? verify)
    +382 short ?, +384 short ?, +386 short country value (networth, rand(3..10)*planet factor; cities +10), +388 short number of factory bays?, +390 short terrain (1 arctic,2 conifer,3 desert,4 volcanic,5 city), +392 short ?, ... up to +235 of the next record.
  (So e.g. `[_MECHRECORD+386]` indexed by bx=c*236 is the value of country c.)
- after countries (0x43f8..): 252 trailing bytes with a few values (year/season/turn?).

Cyborg types (index 0..8): TITAN, BEHEMOTH, HERCULES, ACHILLES, CROSSBOLT, MARS, DRAGON, CYCLOPS, MERCURY.
Tables in CYBOR: _UNITCOST (longs): 100000, 90000, 40000, 30000, 70000, 15000, 10000, 5000, 2500.
_BUILDTIME: 5,4,3,3,4,2,1,1,1 turns; _MFBTMIN: 3,2,1,1,2,1,1,1,1 (min build time with enhancements?).
_FACTCOST longs 500000/800000/1200000, _FACTPOT 1/2/4 (bays per factory), _FACTFILL 3/4/6 (?).
_FORTCOSTS longs 100000/200000/300000. _REPFACTOR floats .6 .55 .5 .45 .4 .35 .3 .2 .1 .05 0 (repair cost fraction by status decile).
_CITYVALUE: city country numbers. _STARTINGLOCATION: 5 rows x 3 candidate start countries. _SCTRY/_SMAPX/_SMAPY: start scroll positions.
_ST (9 x 8 shorts) per-cyborg stats; _WSTS (8 x 4) weapon stats; _CYBWEAP (9 x 12) weapons per cyborg; _CSTS; _HL armor list 750..30.
Terrain types: 1 ARCTIC, 2 CONIFEROUS, 3 DESERT, 4 VOLCANIC, 5 CITY. Fort levels: 0 none, 1 light (4 LAC + 4 LRM, 100000),
2 medium (4 HAC + 6 LRM, 200000), 3 heavy (4 NC + 8 LRM, 300000). Factory: uni-post (cap 1, 500000), dual-post (2, 800000), quadro-post (4, 1200000). Factory upgrade/enhancement 400000. Capital/government 30000.
ENG.DAT strings (UI text) are in the file; offsets listed in re/eng_strings.txt.

## Battle map (ACTION.PRG)
Battlefield = 80x78 tiles, tile indices 0..179 (0-59 terrain tileset GIF, 60-119 STRUCT4.GIF, 120-179 WALL4/CITY2F),
each tileset GIF is 320x200 with 32x32 tiles, 10 per row, 6 rows. Border of the map = tile 2, 3 tiles thick.
The .MAP files (really GIF87a images) hold the 80x78 tile map in their top-left pixels. MATRIX.DAT (80x78 bytes) is the
composed battlefield written by CYBOR.PRG (_WRITE_MATRIX) and read by ACTION.PRG. ACTSTUF.IAT (486 bytes = _GAMESTUFF) and
PINFO.IAT / PASS2.DAT carry data between INTRO.PRG → CYBOR.PRG → ACTION.PRG → back.
MECHSET8.GIF holds mech sprites (16 directions), NEWMARS*.GIF, BATFRAME.GIF the HUD frame.
