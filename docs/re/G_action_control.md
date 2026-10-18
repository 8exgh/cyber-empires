# ACTION.PRG — player control, computer mech AI, HUD/radar and rendering

Source: `action.asm` (annotated iced-x86 disassembly of ACTION.PRG) + `re_action_tables.txt` + KEYBOARD.IAT +
BATFRAME.GIF / MECHSET8.GIF / NEWMARS*.GIF. Function dumps used for this analysis are in `re/dump/A/*.txt`
(made with `re/afnc2.py NAME`, which collapses push/call sequences into `CALL F(args)`).

Conventions: all coordinates are pixels unless stated; the battlefield is 80x78 tiles of 32x32 px = 2560x2496 px.
Direction indices 0..7 = N, NE, E, SE, S, SW, W, NW (screen up = N):
`_DIR[d] = {(0,-1),(1,-1),(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1)}` (shorts, 4 bytes per entry).
`side` = 0 attacker, 1 defender. Mech slots: 0..9 attacker cyborgs, 10..19 defender cyborgs, 20..31 fort
turrets (side stored per record). Timing: the game clock `_CLOCK()` is a counter incremented 18.2 times/s by the
reprogrammed timer interrupt (PIT divisor 256 → 4661 Hz; every 32nd IRQ runs the sound sequencer, every 8th of
those (= 18.2 Hz) increments the clock and polls the mouse driver). One **tick** = 1/18.2 s. The frame loop
renders once and then simulates as many ticks as elapsed since the previous frame (`_CONTROL`).

Important correction to the task brief: `_DOCLICK` is **not** mouse handling. It is the per-tick simulation step for
all 32 mechs plus the bullet update (movement, collisions, fire, heat, bullet motion, wall damage). The mouse is
never used in the battle (the generic timer ISR keeps `_EV_BMX/_EV_BMY/_EV_BBUTTON` updated but nothing in
ACTION reads them). All command input is keyboard + 1 or 2 joysticks.

---------------------------------------------------------------------------------------------------------------

## 0. Interfaces with the simulation core (other agent)

* `_MAIN` frame loop (per frame): `SETPLAYER(0);SETMAP(); SETPLAYER(1);SETMAP(); over=DRAW_ALL_MECHS();
  DRAW_BULLETS(); PLOTHIT(CURMECH[0],0); PLOTHIT(CURMECH[1],1); PLOTHEAT(); SETPLAYER(0);DISPLAYMAP();
  SETPLAYER(1);DISPLAYMAP(); CHECKHOLDS(0); CHECKHOLDS(1); CONTROL(); if(EGA) REFRESH(); TOGGLE_SCREENS2();
  DOALLRADAR();` until `over != 0` (DRAW_ALL_MECHS returns -1 when a side has no mech left to switch to).
* `_CONTROL` → `_DOCLICK` (one call per elapsed tick) → per mech `_GETCOMP` (AI dispatcher) →
  `_DOMECH/_DOTUR/_DOMISSLE` which only write the two per-mech command arrays `_JOY[32]` (wanted direction 0..7,
  -1 = none) and `_KEY[32]` (1 = fire). The human's current mech gets `_JOY/_KEY` from `_CONTROL`
  (`_JOYSTICK` + `_BUTTON`). `_DOCLICK` then consumes `_JOY/_KEY` identically for human and AI mechs.
* Core functions used but not re-analysed here: `_FIREWEAPON(m)`, `_ADDHEAT(m,h)`, `_LCHECK(x,y,m)`,
  `_TESTC(a,b)`, `_DCHECK`, `_CLEARMISSLE(bullet,flag)`, `_DELETEBULLET`, `_ADDBULLET`, `_DESTROY`, `_TESTSHIT`.
  Their interfaces are described where the control/AI code depends on them.

### 0.1 Mech record `_MECHS` (32 records x 0x4E = 78 bytes)

| off | size | meaning |
|---|---|---|
| +0x00 | short | x (pixel, sprite centre) |
| +0x02 | short | y |
| +0x04 | byte | number of this mech's bullets currently in flight (inc in `_FIREWEAPON`, dec when a bullet is freed). Fire is refused when >= 16 |
| +0x05 | byte | obstacle-avoidance heading (0..7) |
| +0x07 | byte | obstacle-avoidance turn sense (+1 / -1) |
| +0x08 | byte | count of "blocked by a friendly" events (`_TESTPOSITION`) |
| +0x09 | 4 x short | bullet pointers of guided missiles (weapon 7) in flight, barrel 0..3 |
| +0x11 | 4 x byte | barrel index for each missile above |
| +0x19 | short | target: >=0 mech index; -1..-26 = structure (index = -t-1); -40 (0xFFD8) = no target |
| +0x1B | short | distance to target as computed by `_GETD` (sum of axis distances beyond the 7-px dead zone) |
| +0x1D | short | number of guided missiles currently in flight (steered by `_JOY`) |
| +0x1F | short | reload counter (ticks until the mech may fire again) |
| +0x21 | short | type 0..8 cyborg (TITAN, BEHEMOTH, HERCULES, ACHILLES, CROSSBOLT, MARS, DRAGON, CYCLOPS, MERCURY), 9..12 turret (9 LAC, 10 HAC, 11 PPC/"NC", 12 LRM battery); -1 empty |
| +0x23 | short | facing 0..7 |
| +0x25 | short | last `_JOY` direction applied |
| +0x27 | short | status: 1 moving, 0 standing/blocked, -1 dead or empty slot, -2 exploding, -4 retreated off the map |
| +0x29 | short | -3 = heat shutdown (set by `_ADDHEAT` when heat > 100; cleared when heat < 75), else 0 |
| +0x2B | short | side (0/1) |
| +0x2F | short | heat 0..150 |
| +0x31 | short | step counter (++ every tick; reset to 0 when firing; indexes firing/explosion animation) |
| +0x33 | short | 1 if standing on tile 53 (0x35) |
| +0x37 | short | selected weapon slot 0..3 |
| +0x3B | short | hold-fast flag (written every frame by `_CHECKHOLDS`; initialised 1 by `_BOOTMECH`) |
| +0x3D | byte | obstacle-avoidance step counter (0 none, -1 "just hit border") |
| +0x3E | byte | bump target: -1 none; 0..31 = enemy mech we collided with; otherwise = -(tile id) of a wall we decided to shoot |
| +0x3F,+0x41 | short | bump-target x,y |
| +0x43 | 4 x short | ammo per slot (-1 = unlimited, 0 = none/empty slot) |
| +0x4B | short | armor (hit points) |
| +0x4D | byte | countdown after a guided missile detonated while the view followed it (re-centres the view on the mech when it reaches 0) |

`_BOOTMECH(m)` (from `_INITMECHS`): ammo[s] = `_STATMECH[type].ammo[s]`; slot=0; step=0; hold=1; bump=-1;
bullets-in-flight=0; if status==0 → status=1. `_LOADCRUD` fills x,y,type,side,facing from ACTSTUF and
status=0 / armor = maxarmor*health%/100 (type -1 → status -1, armor 1).

### 0.2 Per-type stats `_STATMECH` (13 x 18 shorts = 36 bytes), fields used by this part

`[0]` feet animation index (13 = `_MED_FEET` types 0..5, 14 = `_LIGHT_FEET` 6..8, -1 turrets),
`[1]` max armor (750,600,390,270,150,150,90,90,30, 60,60,90,30), `[2]` speed px/tick (2,2,2,4,2,4,4,5,6, 0,0,0,0),
`[3..6]` weapon id per slot (-1 none), `[7..10]` ammo per slot (-1 unlimited), `[11..14]` shots (barrels) per slot
(0 = slot absent), `[15]` cooling divisor (50,75,78,90,75,75,30,30,10, 10,10,75,10), `[16]` ? (5,4,3,3,2,2,1,1,1,1,1,1,1),
byte `[34]` sprite size and byte `[35]` collision radius (packed short: TITAN 0x1620 → size 32 / radius 22;
BEHEMOTH, CROSSBOLT, MARS 0x1220 → 32/18; HERCULES, ACHILLES 0x1420 → 32/20; DRAGON, CYCLOPS, MERCURY 0x0E10 → 16/14;
turrets 0x1020 → 32/16).
Weapon ids: 0 heavy laser, 1 PPC, 2 light laser, 3 heavy flamer, 4 light autocannon, 5 heavy autocannon,
6 short-range missile, 7 long-range guided missile (from `_TRANS_TABLES` names, e.g. TITAN slots = PPC, HAUTO, LMIS).

### 0.3 `_WEAPONS` (8 x 11 shorts = 22 bytes) fields used here

`+0` range (700,700,250,80,350,350,400,3200), `+2` bullet speed (16,16,16,8,6,6,10,15), `+4` damage,
`+6` heat to firer, `+8` heat inflicted on target, `+10` HUD icon sprite id (1243,1244,1253,1248,1249,1247,1245,1246),
`+12` reload ticks (3,7,2,1,2,2,4,4), `+16` impact animation offset (25+v: 1→26 `_HLASER_EXPLOSION`, 2→27 `_EXPLOSION`,
0→25 `_DITTY`), `+14/+18/+20` sound ids. Bullet sprite animation = `15 + weapon id` (see §4).

### 0.4 Bullet record (15 bytes, pool `_BULLETS` 800 entries, linked list head `_FPTR`, free list `_NULLPTR`, terminator `_NULLRECORD`)

`+0` next, `+2` x, `+4` y, `+6` dx (byte), `+7` dy (byte), `+8` dir 0..7 (byte), `+9` weapon id (byte),
`+A` owner mech (byte), `+B` frame counter (short: >=0 flying, <0 exploding, counts up to -1 then deleted),
`+D` remaining range.

### 0.5 ACTSTUF.IAT = `_GAMESTUFF` (486 bytes), as read/written by ACTION

| off | content |
|---|---|
| +0 | 32 x 8 bytes: short x, short y, byte type (-1 empty), byte health %, byte facing, byte side |
| +288 | short terrain type `_TTYPE` 0..4 (arctic, conifer, desert, volcanic, city) — selects tileset names, `_COL` ranges, palette |
| +290/+291 | byte `_PSTAT[0]`, `_PSTAT[1]`: 0 = human, -1 = computer (CYBOR: `-(CHOICE&2)/2`, `-(CHOICE&1)`) |
| +292 | result written by `_UNLOADCRUD`: 0 both sides destroyed, 2 attacker has survivors, 1 only defender |
| +293/+294 | attacker / defender retreated flags (any mech with status -4) |
| +295 | 26 structures x 6 bytes: short x, short y (tile top-left pixel), short alive. Structure 0 is weighted as the capital (`_GOALTABLE[goal][1]`) |
| +451/+453 | `_PC1`, `_PC2`: player ids (0..4) of attacker / defender → player colours |
| +455 | `_EGA` flag |
| +457 | aggression index 0..4 → `_ATTAGG = _PRIOR[idx] = {1,1,1,2,3}` |
| +459 | `_ATTGOAL` 0 GUERRILLA, 1 STRUCTURE, 2 CAPITAL, 3 CONQUER |
| +461 | number of joysticks (0,1,2) |
| +463/+465 | initial current mech of side 0 / side 1 (+10 added for side 1) |
| +467 | byte sound type |
| +468..+483 | joystick calibration: A centre x,y, A half-range x,y (`_JOYAX/_JOYAY`), B centre x,y, B ranges |
| +484 | `_HUMPLAYER` = number of human players |

On exit `_UNLOADCRUD` writes back x,y,type,side,facing for all 32 and health% = armor*100/max
(status -1/-2: 101 if armor==0 i.e. destroyed, 0 if an empty slot), the result bytes, and `_MAIN` saves
MATRIX.DAT (damaged map) and ACTSTUF.IAT.

### 0.6 KEYBOARD.IAT = `_J1` (2 key sets x 13 shorts = 26 bytes, scan codes)

Set k (k=0 "attacker/second player", k=1 "first player") at `_J1 + 26*k`:
`[0..7]` move N,NE,E,SE,S,SW,W,NW; `[8]` fire; `[9]` next weapon; `[10]` switch cyborg; `[11]` hold fast;
`[12]` retreat. The shipped file has both sets = `48 49 4D 51 50 4F 4B 47 | 39 38 34 33 32` =
keypad 8,9,6,3,2,1,4,7 | Space, Alt, '.', ',', 'M' (user-edited; the manual's defaults are keypad for player 1 and
Q W E A D Z X C / Space / F1 F2 F3 for player 2). Scan code 1 (Esc) is hard-wired to pause.

---------------------------------------------------------------------------------------------------------------

## 1. Screen layout and palette

### 1.1 Buffers
* `_SCREEN` = VGA A000, 320x200. `_VSCREEN` = 320x200 work buffer (shares `_ACTION1` memory at start-up).
* `_ACTION1`, `_ACTION2`: one **192x192** (6x6 tiles) off-screen battlefield buffer per side (`_ACTSEG[2]`).
  `_MAPINFO[side] = {tile_x, tile_y, pad, pad}` (8 bytes) = top-left map tile held in the buffer.
  `_TOUCH[side][36]` dirty flags (row-major 6x6) for tiles that must be re-blitted from the tilesets.
* Tilesets loaded whole as 320x200 images: `_SHAPES1` terrain (tile ids 0..59), `_SHAPES2` STRUCT (60..119),
  `_SHAPES3` WALL/CITY (120..179); tile t → sheet t/60, cell (t%60)%10*32, (t%60)/10*32.
  Sprite sheets: `_MECHSET` (MECHSET8.GIF, 16x16 cells, 20 per row), `_NEWMARS`, `_NEWMARS2` (32x32 cells, 10 per row).
* BATFRAME.GIF is decoded into `_VSCREEN`, copied to the screen (`_PAGEMOVE`) and faded in; everything else is
  drawn over it each frame. On VGA there is no page flipping (`_TOGGLE_SCREENS2`, `_COPY_SCREENS2/3`,
  `_REFRESH` after `_CONTROL` only act in EGA mode).

### 1.2 Viewports (`_DISPLAYMAP`, `_SETMAP`, `_DRAWMAP`)
* Each side has a **145x139** viewport: side 0 at screen (0,0)-(144,138), side 1 at (175,0)-(319,138)
  (`_DESX` = 0 / 0xAF). The 30-px strip x=145..174 is the HUD tower graphic.
* The view is always centred on the side's current mech: the point tracked is `(cx, cy) = (mech.x - 72, mech.y - 69)`
  (or the lead guided missile's position if `[0x1D] > 0` or `[0x4D] > 0`, i.e. the camera follows the LRM salvo).
  Window origin inside the 192x192 buffer: `ox = cx - MAPINFO.x*32`, `oy = cy - MAPINFO.y*32`.
  `_SETMAP` scrolls the buffer by whole tiles while `ox >= 48` (`_LEFTSCROLL`, MAPINFO.x++, column 5 dirty),
  `ox < 0` (`_RIGHTSCROLL`, MAPINFO.x--, column 0 dirty), `oy >= 54` (`_UPSCROLL`, MAPINFO.y++, row 5 dirty),
  `oy < 0` (`_DOWNSCROLL`, row 0 dirty), redrawing dirty tiles after each step. Hence `ox∈[0,47]`, `oy∈[0,53]`
  and `ox+145 <= 192`, `oy+139 <= 192`. Scrolling is therefore pixel-smooth and the mech sits at viewport
  pixel (72,69) — there is no dead zone.
* `_DRAWMAP(px,py)` (used when switching mech / after a missile view): `MAPINFO = (clamp(px/32-1, 0, 74),
  clamp(py/32-1, 0, 72))`, all 36 tiles dirty, `_REDRAW_TOUCHED`.
* `_DISPLAYMAP`: `DO_BLIT(ACTION→SCREEN, (ox,oy)-(ox+144, oy+138) → (DESX,0), mode 3)` then `_REDRAW_TOUCHED`
  (restores terrain under every sprite drawn this frame, so sprites never persist in the buffer).
* 2-player split: both viewports are always drawn (even when one side is the computer: it simply shows the
  computer's current mech). There is no single-player full-width mode.

### 1.3 HUD (bottom 60 px, y=140..199; all coordinates exact, per side `p` = 0/1)
| element | position | drawn by |
|---|---|---|
| weapon icons (4 slots) | 16x16 at `(58 + 128p + 20s, 145)`, s=0..3; sprite `_WEAPONS[w].icon` or 1241 (empty slot); selected slot gets sprite 1242 (red frame) blitted transparently on top | `_UPWEAPONS` |
| ammo bars | 15x3 at `(58 + 128p + 20s, 164..166)`; filled length = ammo*15/maxammo (unlimited → 15); empty slot → 0 | `_PLOTAMMO` → `_DOBAR2` |
| heat bar | 54x3 at `(79 + 129p, 170..172)`; length = heat*54/150 | `_PLOTHEAT` → `_DOBAR` |
| armor bar | 54x3 at `(79 + 129p, 177..179)`; length = armor*54/maxarmor | `_PLOTHIT` → `_DOBAR` |
| HOLD FAST text | 16x12 from MECHSET (96,146)-(111,157) at `(59+128p, 186)`, plus blank cell MECHSET (304,178)-(319,189) at `(75+128p,186)` | `_DRAWHF` |
| RETREAT text | 32x12 from MECHSET (112,178)-(143,189) at `(59+128p, 186)` | `_DRAWRF` |
| clear indicator | blank cell at `(59+128p,186)` and `(75+128p,186)` | `_DRAWCL` |
| short-range radar | 48x48 black box at `(3 + 269p, 146)`; centre cross at `(26+269p, 169)` | `_DORADAR` |
| long-range radar | 39x39 black box at `(141,146)-(179,184)` | `_DOLONG` |
Bars: filled part colour 9 (palette index 8 = red 0x600), empty part colour 1 (index 0, black). GEM-style colour
numbers are 1-based: `_PSET/_VSF_COLOR` store `colour-1` as the palette index.

Pre-battle screen (`_MAIN` before the loop): `_DRAWSP(56,90)`, `_DRAWSP(229,90)` = "HIT KEY TO START" 32x16
(MECHSET (272,176)-(303,191)); `_DRAWFL(56,20,PC1)`, `_DRAWFL(229,20,PC2)` = player flag tile 32x32 from
STRUCT sheet cell (160+32*pc, 32) (tile id 75+pc); wait `_GETCH`.

### 1.4 Radars
`_DOALLRADAR` (each frame): on VGA first `_REFRESH` erases the previous frame's radar pixels (each `_SPSET` pixel
is pushed on `_STACK`, max 600, and replayed with colour 1=black). Then for p=0,1: `_DORADAR(cx,cy, 3+269p)`
with (cx,cy) = current mech position (or its lead missile); then `_DOLONG`.
* `_DORADAR(cx,cy,xo)`: cross pixels (xo+23,168),(xo+23,170),(xo+24,169),(xo+22,169) colour 5 (index 4, grey 0x444).
  Every mech with status >= 0: `sx = (x-cx)/32 + 23, sy = (y-cy)/32 + 23`; if 0<=sx,sy<=47 plot `(xo+sx, 146+sy)`
  with colour 32 (index 31 = side-0 player colour #3) for mech index < 10, colour 14 (index 13 = side-1 player
  colour #3) for index >= 10 (so turrets always show in the defender colour). Every bullet: colour 9 (index 8, red).
  1 pixel = 1 tile; radius 23 tiles around the mech. `_SPSET` only plots over black pixels (value 0).
* `_DOLONG`: for mechs 0..19 alive: `lx = (x-1280)/70 + 18, ly = (y-1152)/70 + 18`, if 0..38: plot at
  `(141+lx, 146+ly)`: side-0 current mech colour 32 (idx 31), side-1 current colour 14 (idx 13), other 0..9 colour 15
  (idx 14 = side-0 colour #2), other 10..19 colour 13 (idx 12 = side-1 colour #2). (Both "current" pixels are
  overdrawn by the generic colour if the same pixel is black... they are drawn first, so the later plot is skipped
  because the pixel is no longer black — current mechs keep their colour.) Turrets are not shown.

### 1.5 Palette
GIFs are 256-colour; the palette of the **last loaded GIF (BATFRAME.GIF)** is the base (`_TEMPPALLET` 3 bytes/entry,
6-bit VGA). `_MAIN` then overrides, with `_SETAPAL(i, rgb12)`: `R=(rgb>>8)*8, G=((rgb>>4)&15)*8, B=(rgb&15)*8`
(Atari-ST 3-bit colour values 0..7 → 0..56):
* entries 0..15 ← `_COLOUR_PALETTE = {0,0,0,0,0x444,0x222,0,0x760,0x600,0,0x333,0,0,0,0,0}` (after POKEs setting
  0,1,2 to black and entry 7 to 0x1f grey, which 0x760 then overrides);
* entries `_TOFF = {0,1,2,3,6}` ← `_TERRAIN_COLOURS[TTYPE]`: arctic {000,555,333,777,057}, conifer {000,060,040,120,057},
  desert {000,530,420,750,057}, volcanic {000,334,445,224,700}, city {000,555,333,600,057};
* entries `_P1OFF = {11,14,31}` ← `_PLAYER_COLOURS[PC1]`, entries `_P2OFF = {9,12,13}` ← `_PLAYER_COLOURS[PC2]`,
  where `_PLAYER_COLOURS[5][3]` = P0 {111,222,333}, P1 {220,442,664}, P2 {020,040,070}, P3 {003,036,247},
  P4 {222,444,666}.
* `_EPC` (15 shorts) is never referenced in ACTION (EGA leftovers).
Sprites are drawn in palette indices 11/14/31 (the "attacker" slots). Side-1 sprites are blitted with
`_DO_BLIT` mode 15 which remaps 11→9, 14→12, 31→13 (and skips 0) — that is how the defender gets its player colour.
`_DRAW_ALL_MECHS` sets `_ETYPE = 15` for side 1 (and for everything on EGA), 7 otherwise.
`_DO_BLIT(src*, dst*, x1,y1,x2,y2, dx,dy, mode)` (inclusive rectangles, mfdb = {w,h,seg}): mode 0 fill with
`_SCO2-1`, 3 opaque copy, 7 transparent copy (source byte 0 skipped), 15 transparent + remap.

---------------------------------------------------------------------------------------------------------------

## 2. Controls

Per frame `_CONTROL` does, for each side `s` with `_PSTAT[s] != -1` (human):
1. `cur = _CURMECH[s]`; key set `k = s`, except `if (_HUMPLAYER == 1 && joysticks == 0) k = 1` (a lone human
   always uses key set 1, whichever side he plays).
2. `_JOY[cur] = _JOYSTICK(&_J1[k])` = index of the first pressed movement key (0..7) else -1.
3. Joystick A is used by side 0 if human, or by side 1 if it is the only human; joystick B by side 1 when
   2 joysticks are configured. Buttons: button 0 → key state of `key[8]` (fire), button 1 → `key[9]` (next weapon)
   (`_BSTICK(b)` reads port 0x201 bit 4+b, active low; `_SETK(scan, 0xFF/0)` writes the key-state table).
   Axes: `jx = _JOYIN(0) - centreX; jy = _JOYIN(1) - centreY`; `xq = 1; if (JOYAX/2 > jx) xq=0; if (JOYAX*3/2 < jx) xq=2;`
   `yq = 1; if (JOYAY/2 > jy) yq=0; if (JOYAY/4 + JOYAX < jy) yq=2;` (sic — the last test uses JOYAX; original bug)
   `_JOY[cur] = _JSREF[yq*3+xq]` with `_JSREF = {7,0,1, 6,-1,2, 5,4,3}` (overrides the keyboard direction).
4. `_KEY[cur] = _BUTTON(&_J1[k], s)`:
   * Esc (scan 1): wait for release, `_MDOWN` (restore IRQ 8/9), sound off, `_KUP` (keyboard ISR only), wait for
     Esc press+release, `_KDOWN`, `_MBOOT` → **pause**.
   * key[9] weapon → 2, key[10] switch → 3, key[11] hold → 4, key[12] retreat → 5; each is edge-triggered through
     `_LKEY[scan]` (returns -1 while still held). First match wins (weapon > switch > hold > retreat).
   * fire key[8]: if current weapon is 7 (guided LRM) and `_LFIRE[s]==1` and pressed → keep LFIRE=1, FIRE=0 and
     return 0xFF (no fire: the button is being held since the launch). Else `LFIRE[s]=FIRE[s]; FIRE[s]=pressed;`
     return 1 if pressed (auto-fire while held, limited by reload) else 0.
5. Then: `KEY==2 → _BUMPWEAPON(cur,s)`; `KEY==3 → _BUMPMECH(s)`; `KEY==4 → hold toggle`; `KEY==5 → retreat toggle`.

Semantics:
* **Movement** (`_DOCLICK` for the current mech): a direction key/stick makes the mech first *turn* one 45° step per
  tick toward that direction (`_NDIR`), not moving while its facing differs; once facing == direction it moves
  `speed` px per tick in that direction. No key → status 0 (stand). The mech does not move toward a pointer.
* **Fire** (`KEY==1`): fires the selected slot if `reload==0` and `< 16` own bullets in flight; all barrels of the
  slot fire at once (`_FIREWEAPON`), `reload = _WEAPONS[w].reload`, step counter = 0. Weapon 7 launches guided
  missiles; while any are in flight the direction input steers the *missiles* (lead missile turns 45°/tick toward
  `_JOY`, others keep formation via the barrel offsets) and the camera/radar follow the lead missile. A *new* press
  of fire (FIRE==1 && LFIRE==0) while missiles fly detonates all of them (frame = -impact length).
* **Next weapon** (`_BUMPWEAPON`): slot = next slot (mod 4) with `_STATMECH[type].shots[slot] != 0`; redraw icons.
  (Keys 1-4 are not used.)
* **Switch cyborg** (`_BUMPMECH(s)`): `_CFL[cur] = -1` (old mech reverts to AI); search `(cur+1)%20, …` for a mech
  with status >= 0 and side == s; `_SWITCHMECH(new)`: `_CURMECH[s]=new`, `_DRAWMAP` centred on it (or its missile),
  `_CFL[new] = _PSTAT[s]` (0 = human-controlled), `_UPWEAPONS`. Returns -1 if none (side wiped out). Only cyborgs
  0..19 can be controlled, never turrets.
* **Hold fast** (`KEY==4`): `_RETREAT[s]=0; _MH[s] = (_MH[s]==-1) ? 1-_LHO[s] : 1-_MH[s]` (toggle starting from the
  automatically computed state). Effect: see `_CHECKHOLDS` §3.5.
* **Retreat** (`KEY==5`): `_RETREAT[s] ^= 1; _MH[s] = 0;` draw RETREAT text or clear. Effect: all AI mechs of the side
  head in `_RDIR[s]` (§3.4); any mech of the side (human or AI) reaching a border tile (id 2) gets status -4
  (left the battlefield) instead of being blocked.
* There is no "target select" key: targets are chosen automatically (`_SELECTT`) for AI mechs; a human mech's
  `[0x19]` is only used by the camera/`_DOMISSLE` and by `_COMPASS` when it reverts to AI. No HUD mouse buttons.

Low level: `_MBOOT` hooks IRQ 8 (timer, PIT reprogrammed with divisor 0x100) and IRQ 9 (keyboard: port 0x60 →
`cs:[0x21+scan] = 0xFF` on make, 0 on break; `_GETK(scan)` reads it). `_MDOWN` restores both. `_JOYIN(axis)`
counts polls of port 0x201 bit `axis` (max 3000). `_MSC` starts music; `_BCONIN` is the mouse/sprite library (unused
in ACTION apart from the timer tick).

---------------------------------------------------------------------------------------------------------------

## 3. Computer-controlled mech AI

A mech is AI-controlled when `_CFL[m] == -1`; i.e. every mech except the human's current one. The computer side's
"current" mech is also AI (`_CFL = _PSTAT = -1`). All AI output is `_JOY[m]`/`_KEY[m]`, consumed by `_DOCLICK`
exactly like human input (turn-then-move, reload, etc.).

### 3.1 `_GETCOMP(m)` dispatcher (called every tick from `_DOCLICK` before the mech is moved)
```
if (CFL[m] != -1) return;                               // human current mech
t = mech.target;
if ((t >= 0 && MECHS[t].status == -1) || (t < 0 && !structure[-t-1].alive))   // note: t==-40 reads garbage index 39 -> usually "dead" -> reselect
    mech.target = SELECTT(m);
KEY[m] = 0; JOY[m] = -1;
if (mech.missile_view_countdown > 0) return;            // [0x4D]
if (mech.missiles_in_flight) DOMISSLE(m);
else if (mech.type > 8) DOTUR(m);
else DOMECH(m);
```

### 3.2 Target selection `_SELECTT(m)` → mech index, -(struct)-1, or -40
```
bestS = bestM = 32000;
if (side == 0)                                           // only the attacker targets structures
  for i in 0..25 if structure[i].alive:
     d = |sx+16 - x| + |sy+16 - y|;
     w = (i == 0) ? GOALTABLE[goal][1] : GOALTABLE[goal][2];
     d = d / (w * ATTAGG);
     if (d < bestS) { bestS = d; sIdx = -i; }
for j in 0..31 if MECHS[j].status >= 0 && MECHS[j].side != side:
     d = |xj - x| + |yj - y|;
     w = (j < 20) ? GOALTABLE[goal][0] : GOALTABLE[goal][3];
     d = d / (w * ATTAGG);
     if (d < bestM) { bestM = d; mIdx = j; }
if (bestM == 32000) return -40;                          // no enemy mech alive at all (even if structures exist)
if (bestS <= bestM) { _TR = bestS; return sIdx - 1; }   // structure index i -> -(i+1)
_TR = bestM; return mIdx;
```
`_GOALTABLE[4][4] = {cyborg, capital(struct 0), other structures, turrets}`:
GUERRILLA(0) {1,1,1,1}, STRUCTURE(1) {1,2,2,2}, CAPITAL(2) {1,2,1,1}, CONQUER(3) {2,1,1,1}.
A weight of 2 halves the effective distance of that class → preferred. Defenders always use the cyborg/turret
weights only. **Aggression (`_ATTAGG` 1,1,1,2,3) divides every distance by the same factor, so it never changes the
choice; `_TR` is written but never read: aggression/commitment has no effect inside ACTION.PRG.**
Re-selection happens: when the target dies (`_GETCOMP`), every 16 ticks in `_DOMECH` (`ITIME & 15 == 1`), every
tick for LRM turrets (type 12), and whenever the mech is hit by a bullet (`_DOCLICK`).

### 3.3 Heading to target `_COMPASS(m)` / `_GETD`
```
if (RETREAT[side]) return RDIR[side];                   // RDIR[s] = (initial facing of mech 0 / mech 10 + 4) % 8
if (target >= 0) dx = xt - x, dy = yt - y; else dx = sx+16 - x, dy = sy+16 - y;
return GETD(dx, dy, &mech);
GETD: qx = dx > 7 ? 1 : dx < -7 ? -1 : 0  (same qy); mech.dist[0x1B] = (|dx| if |dx|>7) + (|dy| if |dy|>7);
      return DIRTAB[(qy+1)*3 + qx+1];   DIRTAB = {7,0,1, 6,-1,2, 5,4,3}   (-1 when within 7 px on both axes)
```
So the AI steers in 8 directions with a 7-px dead zone; it heads diagonally until one axis is within 7 px.

### 3.4 Cyborg behaviour `_DOMECH(m)` (every tick)
```
if ((ITIME & 15) == 1) target = SELECTT(m);
KEY = 0;
if (target == -40) { JOY = -1; return; }                // nothing to do
cdir = COMPASS(m);                                      // also refreshes mech.dist
if (mech.hold) {                                        // ---- HOLD FAST: stand and shoot
   w = SELECTWEAPON(m); if (w != slot && w != -1) { slot = w; UPWEAP2(m); }
   if (w != -1 && weapon_id(slot w) == 7) KEY = 1;      // LRMs are launched regardless of facing (guided)
   r = range of slot-0 weapon;                          // NB uses slot 0's range for the line-of-fire tests
   fdir = GETTD(m, r, side);  fdist = TRANGE;           // nearest friend in one of the 8 lines of fire
   edir = GETTD(m, r, 1-side); edist = TRANGE;          // nearest enemy
   if (edir == -1) return;
   if (fdist <= edist && fdir == edir) return;          // a friend is in the way
   if (facing != edir) JOY = edir;                      // turn on the spot (DOCLICK only moves once facing==JOY,
   else KEY = 1;                                        //  and next tick we fire instead)
   return;
}
if (mech.bump != -1) {                                  // ---- we collided with an enemy, or chose a wall to shoot
   w = (0 <= bump <= 32) ? SELECTWEAPON(m) : 0;         // (bump is a signed byte: >32 only when it is -(tile) seen unsigned... see below)
   avoid = 0; if (w != slot) { slot = w; UPWEAP2(m); }
   JOY = GETD(bump_x - x, bump_y - y, &mech);           // head straight for it
   if (JOY == facing) KEY = 1;                          // and fire when facing it
   if (bump < 0 || bump > 33) {                         // wall target: tile = -bump
       if (map[bump_y/32][bump_x/32] != (-bump & 0xFF)) bump = -1;   // wall destroyed -> forget
       if (heat <= 75) return; KEY = 0; return;         // stop shooting walls when hot
   } else if (MECHS[bump].status == -1) bump = -1;      // enemy died -> forget
   return;
}
// ---- normal: fire if the target is in the line of fire, move by compass with obstacle avoidance
w = SELECTWEAPON(m); if (w != slot && w != -1) { slot = w; UPWEAP2(m); }
r = range of slot-0 weapon;
fdir = GETTD2(m, r, side, facing); fdist = TRANGE;      // any friend straight ahead within r?
edir = GETTD2(m, r, 1-side, facing); edist = TRANGE;    // is *the target* straight ahead within r?
if (edir != -1 && (fdist > edist || fdir != edir)) KEY = 1;
if (status == 0 && avoid == 0) {                        // blocked last tick and not avoiding yet
   CHECKOBSTICAL(m);                                    // start avoiding: pick a side, turn 45°
   JOY = mech.avoid_heading; return;
}
if (avoid <= 0 || avoid == 32 || friendly_blocks > 32) { // not avoiding, or avoided too long
   avoid = 0; friendly_blocks = 0; JOY = cdir; return;  // go by compass
}
GETCOLLISIONMATRIX(m);                                  // OBS[0] = blocked at heading-1, OBS[2] = blocked at heading+1
if (OBS[1 - turn] == 0) { heading = (heading - turn) & 7; avoid--; }   // side we turned away from is free: turn back
else if (status == 0) CHECKOBSTICAL(m);                 // still blocked: turn further
JOY = heading;
```
Notes: `w == -1` (no weapon in range) leaves the slot unchanged; `SELECTWEAPON` chooses by `_PRIORITIES` (§3.6).
The "bump" byte is tested with signed compares in the code (`<0` / `>0x20`), so a wall target `-(tile)` for tiles
3..127 is negative and tiles >=128 wrap to positive values >0x20 — both branches reach the wall code.

Obstacle avoidance details:
* `_CHECKOBSTICAL(m)`: `avoid++`; on the first call (`avoid==1`) choose the turn sense:
  `turn = _DIR_CORRECT[facing]` (= {1,-1,-1,1,1,-1,-1,1}), negated if `|dy_to_target| < |dx_to_target|`
  (target: mech position or structure corner-16). Then `heading = (facing + turn + 8) & 7`. Each further call while
  still blocked turns another 45° the same way.
* `_GETCOLLISIONMATRIX(m)`: for i in {0,2}: `d = (facing + i + 7) & 7`; `OBS[i] = TESTPOSITION(x + DIR[d].x*speed,
  y + DIR[d].y*speed, m)`.
* `_TESTPOSITION(x,y,m)`: 1 if `LCHECK(x,y,m)` (tile collision at the 4 corners of a box of half-size
  `radius/2 - 4`) or if any other record j (all 32 slots, alive or not!) has `|x-xj| < (rj + rm + 10)/2` and
  `|y-yj| < ...` (radii = `_STATMECH[35]`); when that j is a friend, `friendly_blocks++`.
* In `_DOCLICK` the actual move is cancelled (status=0) if `LCHECK` fails or `_TESTC` overlaps another living mech
  (box half-size `(rm + rj + 10)/2`); `_TESTC` also records an *enemy* overlap as the bump target of both mechs
  (`bump = other, bump_xy = other's position`) if they had none → the AI then rams/fires at point blank.
* `_TESTSHIT` (called by `_LCHECK` per corner): border tile 2 → blocked and `avoid = -1` (or status -4 if the side
  is retreating); a tile inside one of the three `_COL[TTYPE]` ranges → blocked, and if the mech has no bump target
  and `_DESTROY(tile, m)` says so, the wall tile becomes the bump target (`bump = -tile`, bump_xy = tile centre) →
  the AI shoots its way through. `_DESTROY` returns 1 when the mech's target is a structure, or terrain is conifer
  and tile < 60 (trees), or terrain is city and the mech is an attacker with goal GUERRILLA; 0 otherwise.
  `_COL[TTYPE][3 ranges]` = arctic {120..120, 61..93, 120..169}, conifer {3..20, 61..93, 119..169},
  desert/volcanic {120..120, 61..93, 120..169}, city {3..59, 61..93, 168..179}.

Approach/retreat distances: there is **no** explicit stand-off distance per weapon. The mech walks toward the
target until the 7-px dead zone (i.e. it closes to contact) and fires whenever `GETTD2` sees the target straight
ahead within the slot-0 weapon's range; the weapon actually fired is chosen by `SELECTWEAPON` from the distance
`[0x1B]`. Hold-fast mechs never move (they only rotate). Retreating mechs only follow `RDIR`.

### 3.5 Hold fast — `_CHECKHOLDS(side)` (every frame)
```
if (MH[side] == -1) {                                   // automatic
   hold = 0;
   for the side's 10 cyborgs alive: for slot 0..2: if weapon_id(slot)==7 && ammo[slot] != 0: hold = 1;
   for turrets 20..31 alive with this side and type == 12: hold = 1;
} else hold = MH[side];                                 // manual override (hold key)
for the side's 10 cyborgs: mech.hold = hold;            // dead ones too; harmless
if (hold != LHO[side] && !RETREAT[side]) draw HOLD FAST (hold) or clear;
LHO[side] = hold;
```
So a side automatically holds fast while it still has LRM ammunition (cyborgs stand and launch missiles at long
range, turning to face; LRM turrets count as well); the hold key toggles relative to that state; retreat clears it.
`_MH[2] = {-1,-1}` initially, `_LHO = {-1,-1}`, `_RETREAT = {0,0}`.

### 3.6 Weapon choice `_SELECTWEAPON(m)` → slot or -1
```
for k in 0..3: s = PRIORITIES[type][k]; if (s == -1) break;
    if (ammo[s] != 0 && WEAPONS[weapon_id(s)].range > mech.dist) return s;
return -1;
```
`_PRIORITIES[9][4]` = TITAN {1,0,2,-1}, BEHEMOTH {2,0,3,1}, HERCULES {2,0,1,-1}, ACHILLES {1,0,-1,-1},
CROSSBOLT {1,0,-1,-1}, MARS {1,0,-1,-1}, DRAGON {1,0,-1,-1}, CYCLOPS {1,0,-1,-1}, MERCURY {0,-1,-1,-1}.
(`_WEAPON_RANGE` {150,150,50,2,15,25,50,0} is not referenced by the code.)

### 3.7 Line-of-fire tests
`_GETTD(m, range, side)` → direction 0..7 of the nearest record of `side` (status >= 0, != m) that lies within a
corridor of one of the 8 directions, else -1; `_TRANGE` = its |dx|+|dy|.
Directions equal to `RDIR[m/10]` or its two neighbours are skipped (for cyborgs: never fire toward the own
retreat edge; for turrets m/10 = 2,3 reads beyond `_RDIR` so nothing is skipped). Corridor test for direction s:
`|dx|+|dy| < range`, `| D2[s].y*|dx| - D2[s].x*|dy| | < 32` and `D1[s].x*dx >= 0, D1[s].y*dy >= 0`, with
`_D1[8] = {(0,-1),(1,-1),(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1)}` (byte pairs) and
`_D2[8] = {(0,1),(1,1),(1,0),(1,1),(0,1),(1,1),(1,0),(1,1)}` — i.e. orthogonal directions need the perpendicular
offset < 32 px, diagonals need `||dx|-|dy|| < 32`.
`_GETTD2(m, range, side, facing)`: same corridor test but only for direction = facing and corridor width 48; when
`side` is the enemy side it tests only the mech's current target (mech, or structure centre), when it is the own
side it tests all friendly records.

### 3.8 Turrets `_DOTUR(m)` (types 9..12, speed 0)
```
if (type == 12) { target = SELECTT(m); if (target != -40) KEY = 1; return; }   // LRM battery: fire at anything
fdir = GETTD(m, range(slot 0), side); fdist = TRANGE;
JOY  = GETTD(m, range(slot 0), 1-side); edist = TRANGE;      // rotate toward the nearest enemy in a line of fire
if (JOY == -1) return;
if (fdist <= edist && JOY == fdir) return;                    // friend in the way
KEY = 1;                                                      // fires only when facing == JOY (DOCLICK turns first)
```

### 3.9 Guided missiles `_DOMISSLE(m)` (AI with LRMs in flight)
Lead missile `b = mech.missile[0]`: `dx = tx - b.x, dy = ty - b.y` (structure: corner+16); quantise with the 7-px
dead zone and `JOY = DIRTAB[...]`. `_DOCLICK` applies it: after the missile has flown > 10 ticks,
`b.dir = NDIR(b.dir, JOY)`; `b.dx = FIRESTUF[dir].x * 15 / 10`, `b.dy = FIRESTUF[dir].y * 15 / 10`
(`_FIRESTUF[8] = {(0,-10),(7,-7),(10,0),(7,7),(0,10),(-7,7),(-10,0),(-7,-7)}`, 15 = `_WEAPONS[7].speed`); the other
missiles of the salvo are placed at `lead + (barrel offset[k][dir] - barrel offset[0][dir])` with the same velocity.

---------------------------------------------------------------------------------------------------------------

## 4. Sprite rendering model

### 4.1 Sprite ids (`_DRAW_SPRITE(x, y, id, dst*)` draws centred on (x,y), returns the size)
* `id / 1024 == 1` → MECHSET8.GIF, 16x16 cells: `c = id - 1024; src = ((c % 20)*16, (c / 20)*16)`; drawn at (x-8, y-8).
* `id / 1024 == 0` → NEWMARS.GIF, 32x32 cells: `c = id - 1; src = ((c % 10)*32, (c / 10)*32)`; drawn at (x-16, y-16).
* `id / 1024 == 2` → NEWMARS2.GIF, 32x32 cells: `c = id - 2048`.
Blit mode = `_ETYPE` (7 transparent, 15 transparent+recolour for side 1, 3 opaque in `_MAKESIZES`). If the sprite is
fully inside the 192x192 buffer (`x>7,y>7,x<w-7,y<h-7` for 16 px; 15 for 32 px) `_DO_BLIT` is used, otherwise
`_DOCBLIT` clips the rectangle against the destination.
`_DRAW_SPRITE2(x, y, id, dst, mode)` = 16x16 MECHSET cell drawn with top-left at (x,y) (HUD icons).

### 4.2 Animation tables `_ANIMATION[30]` = {frames*, n = entries per direction, 0, extra}
| idx | table | n | extra | content |
|---|---|---|---|---|
| 0 | `_TITAN_BODY` | 9 | 2 | per dir: standing, -1, firing f0..f4, 0, 0 (NEWMARS2 2052+3d, 2053+3d, 2054+3d) |
| 1 | `_BEL_BODY` | 2 | 0 | 33+d (NEWMARS) |
| 2 | `_HERC_BODY` | 9 | 2 | 1+3d standing, 2+3d/3+3d firing (NEWMARS) |
| 3 | `_ACHIL_BODY` | 2 | 0 | 49..52 for d=0..3, 2048..2051 for d=4..7 |
| 4 | `_CROSS_BODY` | 2 | 0 | 25+d |
| 5 | `_MARS_BODY` | 8 | 2 | 41+d standing; firing 2076+d, 2084+d |
| 6 | `_DRAG_BODY` | 2 | 0 | 1024+d (16 px) |
| 7 | `_CYC_BODY` | 2 | 0 | 1032+d (16 px) |
| 8 | `_MERC_BODY` | 2 | 0 | 1256,1257,1258,1259,1260,1254,1255,1209 |
| 9,10,11 | `_HTURRET_BODY` | 4 | 2 | 2092+2d |
| 12 | `_LTURRET_BODY` | 2 | 0 | 2093+2d |
| 13 | `_MED_FEET` | 11 | 0 | walking cycle, 16 px, dir d: {A,B,C,0,B,A,D,E,0,D,-1} with A..E = 1164+5d'.. (see table) |
| 14 | `_LIGHT_FEET` | 7 | 0 | {A,B,0,A,C,0,-1} |
| 15..22 | bullets of weapon 0..7: `_HLASER`, `_PPC`, `_LIGHT_LASER`, `_EXPLOSION`(flamer), `_LIGHT_AUTO`, `_HEAVY_AUTO`, `_MISSILE`(8,extra 4), `_LONG_MISSILE`(9, extra 4) | 2 | 0 | one sprite per direction (laser/autocannon tables repeat 4 sprites twice: dirs d and d+4 share a sprite) |
| 23, 27 | `_EXPLOSION` | 13 | 0 | 1151,1151,1152,1152,…,1156,1156,-1 (12 frames, same for all 8 dirs) |
| 24 | `_MECH_EXPLOSION` | 17 | 0 | NEWMARS 53,53,54,54,…,60,60,-1 (16 frames, 32 px) |
| 25 | `_DITTY` | 5 | 0 | 1159..1162 |
| 26 | `_HLASER_EXPLOSION` | 7 | 0 | 1236,1236,1237,1238,1239,-1 |

`_DRAW_ANIMATION(x, y, anim, dir, moving, part, counter)`:
```
if (anim == -1) return;
for s in 0..1 (both side buffers) if tile (x/32, y/32) is inside MAPINFO[s] .. +5:
   n = ANIM.n; e = ANIM.extra;
   if (part == 0) { k = e ? e : n;  i = dir*n + (counter % (k-1)) * moving; }          // "standing/flying" part
   else           { k = n - e;      i = dir*n + (counter % (k-1)) * moving + e; }      // "firing/launch" part
   id = frames[i]; if (id == -1 || id == 0) id = frames[i-1];                           // 0/-1 = repeat previous
   size = DRAW_SPRITE(x - MAPINFO[s].x*32, y - MAPINFO[s].y*32, id, ACTSEG[s]);
   mark TOUCH[s] for the tiles containing (x±size/2, y±size/2).
```
(`k-1 == 0` cannot happen for the tables above except n=2,e=0 → k=2 → counter%1 = 0.)

### 4.3 Mech drawing (`_DRAW_ALL_MECHS`, each frame, slots 0..31 in order, into both side buffers)
```
for m: switch status:
  -4: if m is its side's current mech: BUMPMECH(side) == -1 → return -1 (battle over); not drawn.
  -1: skip.
  -2 (exploding): big mechs (type < 6): DRAW_ANIMATION(x,y, 24, 0, 1, 0, step) until step > 16 → status = -1;
                  others: DRAW_ANIMATION(x,y, 23, 0, 1, 0, step) until step > 12 → status = -1;
                  when it dies and was current → BUMPMECH; -1 → return -1.
  else: ETYPE = (side == 1 || EGA) ? 15 : 7;
        if (!on_tile_53) DRAW_ANIMATION(x,y, STATMECH[type].feet, facing, status /*0 or 1*/, 0, CLOCK());
        if (reload != 0 && slot == 0) DRAW_ANIMATION(x,y, type, facing, 1, 1, step);   // firing frames (slot 0 only)
        else                          DRAW_ANIMATION(x,y, type, facing, 0, 0, step);   // standing frame
        ETYPE = 7;
return 0;
```
Feet are drawn first (walk cycle advanced by the 18.2 Hz clock only while status==1), then the body on top.
Feet are hidden on tile 53 (e.g. water/lava). Turrets have no feet. The body uses the "firing" frames only for
weapon slot 0 (TITAN/HERCULES/MARS/HTURRET have them; the rest just repeat the standing sprite).
The step counter is 0 right after firing so the firing animation starts from its first frame.

### 4.4 Bullets (`_DRAW_BULLETS`, after the mechs)
```
for each bullet b: w = b.weapon;
  if (b.frame < 0)      DRAW_ANIMATION(b.x,b.y, 25 + WEAPONS[w].impact, 0, 1, 0, ANIM[25+impact].n + b.frame); // explosion
  else if (b.frame < 4) DRAW_ANIMATION(b.x,b.y, 15 + w, b.dir, 1, 1, b.frame);   // launch frames (missiles: smoke 1096..)
  else                  DRAW_ANIMATION(b.x,b.y, 15 + w, b.dir, 1, 0, b.frame);   // flying frames
```
Explosion of a bullet: `frame = -(n of impact animation)` set in `_DOCLICK`; it is drawn with counter n+frame = 0,1,…
and deleted when frame reaches -1.

### 4.5 Muzzle positions `_TRANS_TABLES[13][4]` → per-slot table of `[barrel][dir] = (x,y)` shorts (32 bytes per barrel)
Type 0: TITAN_PPC(2 barrels), TITAN_HAUTO(2), TITAN_LMIS(2); 1: BEL_HLAS(4), BEL_SMIS, BEL_LMIS, BEL_LAUTO;
2: HERC_PPC, HERC_HLAS, HERC_SMIS; 3: ACHIL_HLAS, ACHIL_LAUTO; 4: CROSS_LLAS, CROSS_LMIS; 5: MARS_PPC, MARS_LAUTO;
6: DRAG_LLAS, DRAG_HFIRE; 7: CYC_LLAS, CYC_LAUTO; 8: MERC_LLAS(3); 9,10: MTURRET; 11: HTURRET; 12: LTURRET.
`_FIREWEAPON` spawns barrel k's bullet at `(x - size/2 + 8 + tbl[k][facing].x, y - size/2 + 8 + tbl[k][facing].y)`
with velocity `FIRESTUF[facing] * speed / 10` and range `WEAPONS[w].range * (|fx|+|fy| or 14) / 10`.

### 4.6 Hit boxes `_MSIZE[13][8] = {l,t,r,b}` (computed at start-up by `_MAKESIZES`)
For each type and direction the standing sprite is rendered opaque into `_VSCREEN` centred at (size/2,size/2)
and scanned: `l = first non-zero column - size/2 - 1`, `t` likewise, `r = last column - size/2 + 1`, `b` likewise.
`_DOCLICK` uses the box `(x + l - 8, y + t - 8) .. (x + r + 8, y + b + 8)` for bullet collision.

---------------------------------------------------------------------------------------------------------------

## 5. Pseudo-C per function (constants exact)

```c
/* ---------- globals ---------- */
short JOY[32], KEY[32], CFL[32];      // per mech: wanted dir (-1 none), command (1 fire,2 weapon,3 switch,4 hold,5 retreat), control flag (-1 AI, PSTAT otherwise)
short CURMECH[2], PSTAT[2], MH[2]={-1,-1}, LHO[2]={-1,-1}, RETREAT[2], RDIR[2];
char  FIRE[2], LFIRE[2]; char LKEY[128]; short ITIME; short LTIME; short PLAYER, DESX;
short MAPINFO[2][4]; char TOUCH[2][36]; short MSIZE[13][8][4];

/* ---------- _MAIN (set-up part) ---------- */
load actstuf.iat -> GAMESTUFF; pinfo.iat -> P (P[0] saved, zeroed, written back: "battle in progress" marker);
EGA = GAMESTUFF+455; SCRBOOT; SETFADE(0); LOADCRUD(); SET_MFDB();
palette entries 0,1,2 = 0; entry 7 = 0x1f; MBOOT(); load keyboard.iat -> J1;
load MECHSET8, NEWMARS, NEWMARS2, STRUCT*, WALL*, terrain GIF (names from _FNAMES[TTYPE]) into their buffers;
MAKESIZES(); load BATFRAME into VSCREEN; palette overrides (§1.5); WRITEPAL; PAGEMOVE(VSCREEN->A000); FADE_IN;
INITMAP() (matrix.dat -> map, HITLOC[i] = WALL_POWER[map[i]]); INITMECHS(); SETPLAYER(0); SETMAP(); SETPLAYER(1); SETMAP();
DRAW_ALL_MECHS(); DRAW_BULLETS(); PLOTHIT x2; PLOTHEAT(); DOALLRADAR(); DISPLAYMAP x2;
sound init; DRAWSP(56,90); DRAWFL(56,20,PC1); DRAWFL(229,20,PC2); DRAWSP(229,90); MDOWN(); GETCH(); MBOOT(); COPY_SCREENS2();
LTIME = CLOCK(); frame loop (§0); on exit: fade out, write matrix.dat, clear screen, UNLOADCRUD, write actstuf.iat, restore pinfo.iat.

/* ---------- _CONTROL ---------- */
void CONTROL() {
  while (CLOCK() == LTIME) ;                       // wait at least one tick
  n = CLOCK() - LTIME; for (i = 1; i <= n; i++) DOCLICK(); LTIME = CLOCK();
  for (s = 0; s < 2; s++) {
    cur = CURMECH[s]; k = s; if (HUMPLAYER == 1 && GAMESTUFF.joysticks == 0) k = 1;
    JOY[cur] = JOYSTICK(&J1[k]);
    if (PSTAT[s] == -1) continue;
    useA = (s == 1 && PSTAT[0] == -1 && PSTAT[1] != -1) || (s == 0 && PSTAT[0] != -1);
    if (useA && joysticks > 0) { SETK(J1[k].fire, BSTICK(0) ? 0 : 0xFF); SETK(J1[k].weapon, BSTICK(1) ? 0 : 0xFF);
        jx = JOYIN(0) - GS[468]; jy = JOYIN(1) - GS[470];
        xq = 1; if (JOYAX/2 > jx) xq = 0; if (JOYAX/2 + JOYAX < jx) xq = 2;
        yq = 1; if (JOYAY/2 > jy) yq = 0; if (JOYAY/4 + JOYAX < jy) yq = 2;
        JOY[cur] = JSREF[yq*3 + xq]; }
    if (s == 1 && PSTAT[1] != -1 && joysticks == 2) { same with BSTICK(2),(3), JOYIN(2),(3), GS[476],[478], JOYBX, JOYBY }
    KEY[cur] = BUTTON(&J1[k], s);
    if (KEY[cur] == 2) BUMPWEAPON(cur, s);
    if (KEY[cur] == 3) BUMPMECH(s);
    if (KEY[cur] == 4) { RETREAT[s] = 0; MH[s] = (MH[s] == -1) ? 1 - LHO[s] : 1 - MH[s]; }
    if (KEY[cur] == 5) { RETREAT[s] ^= 1; MH[s] = 0; if (RETREAT[s]) DRAWRF(s); else DRAWCL(s); }
  }
}
int JOYSTICK(short *keys) { for (i = 0; i < 8; i++) if (GETK(keys[i])) return i; return -1; }

int BUTTON(short *keys, int s) {
  if (GETK(1)) { while (GETK(1)); MDOWN(); DOOROFF(); KUP(); while (!GETK(1)); while (GETK(1)); KDOWN(); MBOOT(); }
  for (j = 9; j <= 12; j++) {                      // 9 weapon,10 switch,11 hold,12 retreat
    if (GETK(keys[j])) { if (LKEY[keys[j]]) return -1; LKEY[keys[j]] = 1; return j - 7; }
    LKEY[keys[j]] = 0; }
  m = CURMECH[s]; w = STATMECH[type(m)].weapon[slot(m)];
  if (w == 7 && LFIRE[s] == 1 && GETK(keys[8])) { LFIRE[s] = 1; FIRE[s] = 0; return 0xFF; }
  LFIRE[s] = FIRE[s]; FIRE[s] = 0;
  if (GETK(keys[8])) { FIRE[s] = 1; return 1; } return 0;
}
void BUMPWEAPON(m, s) { do slot = (slot + 1) % 4; while (STATMECH[type].shots[slot] == 0); UPWEAPONS(m, s); }
int BUMPMECH(s) { CFL[CURMECH[s]] = -1; start = n = (CURMECH[s] + 1) % 20;
  do { if (MECHS[n].status >= 0 && MECHS[n].side == s) { CURMECH[s] = n; SETPLAYER(s); SWITCHMECH(n); return 0; }
       n = (n + 1) % 20; } while (n != start); return -1; }
void SWITCHMECH(m) { CURMECH[PLAYER] = m;
  if (missiles(m) == 0) DRAWMAP(x - 72, y - 69); else DRAWMAP(missile[0].x - 72, missile[0].y - 69);
  CFL[m] = PSTAT[PLAYER]; UPWEAPONS(m, PLAYER); }
void SETPLAYER(p) { PLAYER = p; ACTION.seg = (p ? ACTION2 : ACTION1).seg; DESX = p ? 0xAF : 0; }

/* ---------- _DOCLICK: one simulation tick ---------- */
void DOCLICK() {
  ITIME++;
  for (m = 0; m < 32; m++) { M = &MECHS[m];
    if (M->mview > 0 && --M->mview == 0 && M->missiles == 0) { SETPLAYER(M->side); DRAWMAP(M->x - 72, M->y - 69); }
    tile = map[(M->y/32)*80 + M->x/32];
    if (M->status == -1) goto tail;
    GETCOMP(m); type = M->type;
    if (ITIME % 10 == m % 10 && M->status != -2) {          // cooling every 10 ticks
      M->heat -= M->armor / STATMECH[type].cool + 1;
      if (M->on53) { if (TTYPE == 3) ADDHEAT(m, 15); else M->heat -= M->armor / STATMECH[type].cool + 1; }
      if (M->heat < 0) M->heat = 0;
      if (M->heat < 75 && M->shutdown == -3) M->shutdown = 0; }
    M->step++;
    box = { M->x + MSIZE[type][M->facing].l - 8, M->y + ..t - 8, M->x + ..r + 8, M->y + ..b + 8 };
    for each bullet b (frame >= 0, inside box, owner != m, M->status != -2, DCHECK(b.x,b.y, box shrunk by 8, b.dir)):
      dmg = WEAPONS[b.w].damage;
      if (dmg > M->armor) { M->status = -2; PLAYBOOM(); M->step = 0; M->armor = 0; }
      else { M->armor -= dmg; M->target = SELECTT(m); M->avoid = 0; ADDHEAT(m, WEAPONS[b.w].heatdmg); }
      b.frame = -ANIM[25 + WEAPONS[b.w].impact].n; b.x += b.dx/2; b.y += b.dy/2; if (b.w == 7) CLEARMISSLE(&b, 1);
    if (--M->reload < 0) M->reload = 0;
    if (M->status < 0 || M->shutdown == -3) goto tail;
    if ((m == CURMECH[0] && FIRE[0] == 1 && LFIRE[0] == 0) || (m == CURMECH[1] && FIRE[1] == 1 && LFIRE[1] == 0))
      { for k < M->missiles: M->missile[k]->frame = -ANIM[25 + impact(w)].n; M->missiles = 0; }   // detonate salvo
    if (JOY[m] == -1) M->status = 0;
    else if (M->missiles > 0) {                                // steer the salvo (lead missile b0 = M->missile[0])
      if (b0->frame > 10) { b0->dir = NDIR(b0->dir, JOY[m]); sp = WEAPONS[7].speed;
        b0->dx = FIRESTUF[dir].x * sp / 10; b0->dy = FIRESTUF[dir].y * sp / 10;
        for k = 1 .. missiles-1: bk->x = b0->x + T[bar_k][dir].x - T[bar_0][dir].x; same y; bk->dx,dy,dir = b0's; } }
    else {                                                     // move
      sp = STATMECH[type].speed; M->lastjoy = JOY[m]; M->facing = NDIR(M->facing, JOY[m]); M->status = 1;
      if (M->lastjoy == M->facing) {
        M->x += DIR[JOY[m]].x * sp; M->y += DIR[JOY[m]].y * sp;
        if (LCHECK(M->x, M->y, m)) { undo; M->status = 0; }
        for j < 32 while M->status == 1: if (MECHS[j].status != -1 && != -4 && j != m && TESTC(m, j) == -1) { undo; M->status = 0; } }
    }
    M->on53 = (tile == 53);
    if (tile == 2 && RETREAT[m / 10]) M->status = -4;         // (m/10: turrets 20..31 index RETREAT[2],[3] = garbage: MH/RETREAT neighbours)
    if (KEY[m] == 1 && M->reload == 0 && M->inflight < 16) { FIREWEAPON(m);
        M->reload = WEAPONS[STATMECH[type].weapon[M->slot]].reload; M->step = 0; }
  tail:
    if (M->shutdown == -3 && M->status > 0) M->status = 0;
  }
  /* bullet update */
  for each bullet b (prev tracked for deletion):
    if (b.frame >= 0) { dx = b.dx; dy = b.dy;
      if ((b.w == 7 && b.frame < 10) || (b.w == 6 && b.frame < 4)) { b.x += dx/8; b.y += dy/8; } else { b.x += dx; b.y += dy; }
      b.range -= |dx| + |dy|; }
    tx = b.x/32; ty = b.y/32; tile = map[ty*80 + tx]; b.frame++;
    if (tile == 2 || b.frame == -1) { if (b.w == 7 && tile == 2) CLEARMISSLE(&b, 0); DELETEBULLET(prev); continue; }
    if ((unsigned)b.range > 0xFA00 && b.frame >= 0) { b.frame = -ANIM[25+impact].n; if (b.w == 7) CLEARMISSLE(&b, 1); }
    if (b.frame <= 0) continue;
    if (tile not in COL[TTYPE] ranges) continue;
    hp = HITLOC[idx]; dmg = WEAPONS[b.w].damage;
    if (hp <= dmg) { for i < 26: if (struct[i].alive && struct[i].x/32 == tx && struct[i].y/32 == ty) { struct[i].alive = 0; PLAYCRIT(); }
                     map[idx] = WALLP_table[TTYPE][tile]; HITLOC[idx] = WALL_POWER[map[idx]];
                     for s < 2: if tile inside MAPINFO[s] window: TOUCHED(s, (ty - MAPINFO[s].y)*6 + tx - MAPINFO[s].x); }
    else HITLOC[idx] = hp - dmg;
    b.frame = -ANIM[25+impact].n; if (b.w == 7) CLEARMISSLE(&b, 1);
}
int NDIR(cur, want) { if (cur == want) return cur; d = want - cur; if (d < -4) d += 8; if (d > 4) d -= 8;
                      return d < 0 ? (cur + 7) % 8 : (cur + 9) % 8; }
/* ADDHEAT(m,h): heat += h; if heat > 100 { shutdown = -3; status = 0; } if heat > 150 { heat = 150; status = -2; armor = 0; step = 0; PLAYBOOM; } */
/* CLEARMISSLE(b, flag): owner->missiles--; remove b from owner's missile list (shift down); SETPLAYER(owner side);
   if owner->missiles == 0 && owner is a current mech: flag ? owner->mview = -b->frame : DRAWMAP(owner x-72, y-69) */

/* ---------- _CHECKHOLDS / _GETCOMP / _DOMECH / _DOTUR / _DOMISSLE / _SELECTT / _COMPASS / _GETD / _GETTD / _GETTD2 /
   _SELECTWEAPON / _CHECKOBSTICAL / _GETCOLLISIONMATRIX / _TESTPOSITION: see §3 (the pseudo-code there is complete). */

/* ---------- HUD ---------- */
void PLOTHIT(m, p)  { DOBAR(79 + 129*p, 177, MECHS[m].armor * 54 / STATMECH[type].maxarmor); }
void PLOTHEAT()     { DOBAR(79, 170, heat(CURMECH[0]) * 54 / 150); DOBAR(208, 170, heat(CURMECH[1]) * 54 / 150); PLOTAMMO(0); PLOTAMMO(1); }
void PLOTAMMO(p)    { m = CURMECH[p]; for s < 4: DOBAR2(58 + 128*p + 20*s, 164, ammo[s] ? ammo[s]*15 / STATMECH[type].ammo[s] : 0); }
void DOBAR(x,y,len) { fill(x, y, x+len, y+2, colour 9); fill(x+len, y, x+54, y+2, colour 1); }   // DOBAR2: width 15
void UPWEAPONS(m,p) { for s < 4: w = STATMECH[type].weapon[s]; DRAW_SPRITE2(58+128p+20s, 145, w != -1 ? WEAPONS[w].icon : 1241, SCREEN, 3);
                      if (s == slot) DRAW_SPRITE2(58+128p+20s, 145, 1242, SCREEN, 7); COPY_SCREENS3(); }
void UPWEAP2(m)     { if (m == CURMECH[0]) UPWEAPONS(m, 0); if (m == CURMECH[1]) UPWEAPONS(m, 1); }
void DRAWHF(p) { blit MECHSET (96,146)-(111,157) -> (59+128p,186); blit MECHSET (304,178)-(319,189) -> (75+128p,186); }
void DRAWRF(p) { blit MECHSET (112,178)-(143,189) -> (59+128p,186); }
void DRAWCL(p) { blit MECHSET (304,178)-(319,189) -> (59+128p,186) and -> (75+128p,186); }
void DRAWFL(x,y,pc) { blit SHAPES2 (160+32pc, 32)-(191+32pc, 63) -> (x,y); }    void DRAWSP(x,y) { blit MECHSET (272,176)-(303,191) -> (x,y); }
void SETAPAL(i, rgb) { PAL[i] = { (rgb>>8)*8, ((rgb>>4)&15)*8, (rgb&15)*8 }; }
void SPSET(x,y,c)   { if (PPOINT(x,y) == 0) { PSET(x,y,c); STACK[++STACKP] = {x,y,c}; } }   // STACKP max 599
void REFRESH()      { for i = STACKP..0: PSET(STACK[i].x, STACK[i].y, 1); STACKP = -1; }
/* DORADAR / DOLONG / DOALLRADAR: §1.4 */

/* ---------- map / viewport ---------- */
void DRAWMAP(px,py) { MAPINFO[PLAYER] = { clamp(px/32 - 1, 0, 74), clamp(py/32 - 1, 0, 72) }; all TOUCH[PLAYER] = 1; REDRAW_TOUCHED(); }
void REDRAW_TOUCHED() { for i < 36 if TOUCH[PLAYER][i]: c = i % 6, r = i / 6; t = map[(MAPINFO.y + r)*80 + MAPINFO.x + c];
    sheet = t / 60; sx = (t % 10)*32 (of t%60), sy = ((t % 60) / 10)*32; DO_BLIT(SHAPES[sheet] -> ACTION, (sx,sy)-(sx+31,sy+31) -> (c*32, r*32), 3); TOUCH = 0; }
void TOUCHED(p, i) { if (0 <= i < 36) TOUCH[p][i] = 1; }
void SETMAP() { m = CURMECH[PLAYER]; (cx,cy) = (x-72, y-69) or lead missile (x-72, y-69) if missiles > 0 or mview > 0;
   ox = cx - MAPINFO.x*32; oy = cy - MAPINFO.y*32;
   do { again = 0;
     if (ox >= 48) { LEFTSCROLL(ACTION);  MAPINFO.x++; ox -= 32; TOUCH[r*6+5] = 1 for r<6; REDRAW_TOUCHED(); again = 1; }
     if (ox < 0)   { RIGHTSCROLL(ACTION); MAPINFO.x--; ox += 32; TOUCH[r*6+0] = 1;       REDRAW_TOUCHED(); again = 1; }
     if (oy >= 54) { UPSCROLL(ACTION);    MAPINFO.y++; oy -= 32; TOUCH[30+c] = 1;        REDRAW_TOUCHED(); again = 1; }
     if (oy < 0)   { DOWNSCROLL(ACTION);  MAPINFO.y--; oy += 32; TOUCH[c] = 1;           REDRAW_TOUCHED(); again = 1; }
   } while (again); }
   // LEFTSCROLL: memmove(buf, buf+32, 192*192-32) (shift image left 32 px); RIGHTSCROLL: shift right; UPSCROLL: memmove(buf, buf+32*192, ...); DOWNSCROLL: shift down a row of tiles
void DISPLAYMAP() { (ox,oy) as above; DO_BLIT(ACTION -> SCREEN, (ox,oy)-(ox+144, oy+138) -> (DESX, 0), 3); REDRAW_TOUCHED(); }
int DRAW_SPRITE / DRAW_SPRITE2 / DRAW_ANIMATION / DRAW_ALL_MECHS / DRAW_BULLETS / MAKESIZES: §4.
void DOCBLIT(src,dst, x1,y1,x2,y2, dx,dy, mode) { clip (dx,dy,x2-x1,y2-y1) to dst (w,h): drop if entirely outside; move x1/y1 up for negative dx/dy, cut x2/y2 at the right/bottom edge; DO_BLIT if non-empty. }
```

---------------------------------------------------------------------------------------------------------------

## 6. Open questions

1. `_STATMECH[16]` (values 5,4,3,3,2,2,1,1,1,1,1,1,1) and `[6]`-adjacent fields not used by the control code —
   meaning to be confirmed by the simulation agent (probably weight class / CYBOR cross-reference).
2. `_WEAPON_RANGE`, `_EPC`, `_LLASER_EXPLOSION`, `_ANIMATION[28..29]` are present but unreferenced in ACTION.PRG.
3. `_GETCOMP` tests `structure[-t-1].alive` even for `t == -40` (index 39 → reads `_GAMESTUFF+299+234`, inside the
   joystick calibration area); exact behaviour when "no target" is therefore data dependent (usually re-selects).
4. `_TESTPOSITION` (AI look-ahead) treats dead mechs (status -1) and empty slots as obstacles while `_TESTC` (actual
   move) ignores them; empty slots have the x,y stored in ACTSTUF (whatever CYBOR left there). Reproduce as-is or fix?
5. Turret slots 20..31 use `m/10` as "side" in `RETREAT[m/10]`, `RDIR[m/10]` (`_GETTD`) and `_COMPASS`, which reads
   neighbouring globals (`_MH`, `_ACTSEG`) — benign in practice (turrets never move, values never match 0..7) but
   worth noting for a faithful port.
6. The joystick dead-zone formula uses `JOYAX` in the y test (original bug) — kept in the pseudo-code.
7. Exact pixel content of tile 53 per terrain (the "feet hidden / cooling / volcanic heating" tile) and the structure
   list order (structure 0 = capital) should be cross-checked with CYBOR's `_START`/`_WRITE_MATRIX`.
8. `_PSTAT` semantics (0 human, -1 computer) derived from CYBOR `_START` (`-(CHOICE&2)/2`, `-(CHOICE&1)`) and ACTION's
   use; `_HUMPLAYER` = `_HUMANNUMBER` (count of human players) — confirm with the INTRO/CYBOR agents.
9. `_WRITEPAL` scales palette bytes by `cs:0x2F` (fade level) — the final DAC values after `_SETFADE`/`_FADE_IN`
   are assumed to be the 6-bit values computed by `_SETAPAL` (max 56/63).
